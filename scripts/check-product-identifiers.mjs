#!/usr/bin/env bun
// A textual contract guard; capability and image-selection tests remain the behavioral authority.
import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync, realpathSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { listFirmwarePlans } from './build-firmware.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const policyPath = 'scripts/product-identifier-exceptions.json';
// shortcut: escaped literals, numeric suffixes and unseparated compounds need AST checks if they must be enforced.
const normalize = (text) =>
  text
    .normalize('NFC')
    .replace(/(\p{Lu}+)(\p{Lu}\p{Ll})/gu, '$1 $2')
    .replace(/([\p{Ll}\p{N}])(\p{Lu})/gu, '$1 $2')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

function catalogTerms(root) {
  const plans = listFirmwarePlans(root);
  const registry = Bun.TOML.parse(readFileSync(join(root, 'products/registry.toml'), 'utf8'));
  const labels = [...Object.keys(registry.products), ...Object.keys(registry.boards)];
  for (const id of new Set(plans.map((p) => p.product))) {
    const product = Bun.TOML.parse(
      readFileSync(join(root, 'products', id, 'product.toml'), 'utf8'),
    );
    for (const key of ['short_name', 'display_name', 'model']) {
      if (typeof product[key] !== 'string' || !product[key].trim())
        throw new Error(`Missing catalog ${key}: ${id}`);
      labels.push(product[key]);
    }
    for (const board of Object.values(product.boards)) {
      if (typeof board.display_name !== 'string' || !board.display_name.trim())
        throw new Error(`Missing catalog board display_name: ${id}`);
      labels.push(board.display_name);
      // Compound marketing words also occur without the chip prefix in identifiers and filenames.
      labels.push(...board.display_name.split(/\s+/).filter((word) => /\p{Ll}\p{Lu}/u.test(word)));
    }
  }
  const crates = [
    ...new Set(
      plans.map((p) => relative(realpathSync(root), realpathSync(p.cwd)).split(sep).join('/')),
    ),
  ];
  labels.push(...crates);
  const terms = new Set(
    labels
      .flatMap((label) => {
        const term = normalize(label);
        return [term, term.replaceAll(' ', '')];
      })
      .filter(Boolean),
  );
  return { terms: [...terms].sort(), crates };
}

function scoped(path, crates) {
  if (crates.some((crate) => path === crate || path.startsWith(`${crate}/`))) return false;
  if (path.startsWith('apps/desktop/src-tauri/vendor/')) return false;
  if (
    path === policyPath ||
    /(^|\/)(?:__tests__|tests|test)\//.test(path) ||
    /\.(?:test|spec)\.[^/]+$/.test(path) ||
    /\.md$|(^|\/)(?:Cargo\.lock|bun\.lock|package-lock\.json|pnpm-lock\.yaml|yarn\.lock)$/.test(
      path,
    )
  )
    return false;
  return (
    /^(?:apps\/desktop\/|crates\/|tools\/|scripts\/|\.github\/workflows\/)/.test(path) ||
    [
      'justfile',
      'Cargo.toml',
      'package.json',
      'rust-toolchain.toml',
      '.cargo/config',
      '.cargo/config.toml',
    ].includes(path)
  );
}

// Strip Rust literals/comments only to locate module braces. The scan still checks original text.
function rustStructure(source) {
  const chars = source.split('');
  const blank = (start, end) => {
    for (let i = start; i < end; i++) if (chars[i] !== '\n' && chars[i] !== '\r') chars[i] = ' ';
  };
  for (let i = 0; i < source.length;) {
    const start = i;
    if (source.startsWith('//', i)) {
      i = source.indexOf('\n', i);
      if (i < 0) i = source.length;
    } else if (source.startsWith('/*', i)) {
      let depth = 1;
      i += 2;
      while (i < source.length && depth) {
        if (source.startsWith('/*', i)) {
          depth++;
          i += 2;
        } else if (source.startsWith('*/', i)) {
          depth--;
          i += 2;
        } else i++;
      }
      if (depth) throw new Error('Unterminated Rust block comment');
    } else {
      const raw = /^r(#{0,255})"/.exec(source.slice(i));
      const char = /^'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\r\n])'/u.exec(
        source.slice(i),
      );
      if (raw) {
        const end = source.indexOf(`"${raw[1]}`, i + raw[0].length);
        if (end < 0) throw new Error('Unterminated Rust raw string');
        i = end + raw[1].length + 1;
      } else if (source[i] === '"') {
        i++;
        while (i < source.length && source[i] !== '"') i += source[i] === '\\' ? 2 : 1;
        if (i >= source.length) throw new Error('Unterminated Rust string');
        i++;
      } else if (char) i += char[0].length;
      else {
        i++;
        continue;
      }
    }
    blank(start, i);
  }
  return chars.join('');
}

export function withoutRustTests(source) {
  const structure = rustStructure(source);
  const chars = source.split('');
  const testModule =
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\{/g;
  for (const match of structure.matchAll(testModule)) {
    let depth = 1;
    let end = match.index + match[0].length;
    while (end < structure.length && depth) {
      if (structure[end] === '{') depth++;
      else if (structure[end] === '}') depth--;
      end++;
    }
    if (depth) throw new Error('Unterminated cfg(test) module');
    for (let i = match.index; i < end; i++)
      if (chars[i] !== '\n' && chars[i] !== '\r') chars[i] = ' ';
  }
  return chars.join('');
}

export function checkProductIdentifiers(root) {
  const { terms, crates } = catalogTerms(root);
  const policy = JSON.parse(readFileSync(join(root, policyPath), 'utf8'));
  if (!Array.isArray(policy)) throw new Error('Identifier exceptions must be an array');
  const exceptions = new Map();
  for (const rule of policy) {
    if (
      !rule ||
      typeof rule.path !== 'string' ||
      !scoped(rule.path, crates) ||
      typeof rule.text !== 'string' ||
      !rule.text.trim() ||
      /[\r\n]/.test(rule.text) ||
      typeof rule.reason !== 'string' ||
      !rule.reason.trim() ||
      !Number.isInteger(rule.count) ||
      rule.count < 1
    )
      throw new Error(
        'Each identifier exception needs a scoped path, exact line, positive count and reason',
      );
    const key = JSON.stringify([rule.path, rule.text.trim()]);
    if (exceptions.has(key)) throw new Error(`Duplicate identifier exception: ${rule.path}`);
    exceptions.set(key, { ...rule, seen: 0 });
  }
  const paths = new Set(
    execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {
      cwd: root,
      maxBuffer: 16 * 1024 * 1024,
    })
      .toString()
      .split('\0')
      .filter((path) => path && scoped(path, crates)),
  );
  const violations = [];
  const matches = (text) => {
    const normalized = ` ${normalize(text)} `;
    return terms.filter((term) => normalized.includes(` ${term} `));
  };
  let checked = 0;
  for (const path of [...paths].sort()) {
    const absolute = join(root, path);
    if (!existsSync(absolute)) continue; // Tracked deletions do not contain code to inspect.
    if (!lstatSync(absolute).isFile())
      throw new Error(`Scoped source must be a regular file: ${path}`);
    const pathTerms = matches(path);
    if (pathTerms.length) violations.push(`${path}: filename contains ${pathTerms.join(', ')}`);
    const data = readFileSync(absolute);
    if (data.includes(0)) continue;
    checked++;
    const source = path.endsWith('.rs') ? withoutRustTests(data.toString()) : data.toString();
    for (const [index, text] of source.split('\n').entries()) {
      const hits = matches(text);
      if (!hits.length) continue;
      const rule = exceptions.get(JSON.stringify([path, text.trim()]));
      if (rule) rule.seen++;
      else violations.push(`${path}:${index + 1}: ${hits.join(', ')}: ${text.trim()}`);
    }
  }
  for (const rule of exceptions.values()) {
    if (rule.seen !== rule.count)
      violations.push(
        `${rule.path}: exception expected ${rule.count} occurrence(s), found ${rule.seen}: ${rule.text.trim()}`,
      );
  }
  return { violations, checked, terms: terms.length, exceptions: exceptions.size };
}

export function main(root = repo) {
  const result = checkProductIdentifiers(root);
  if (result.violations.length) {
    console.error(`Product identifier guard FAILED:\n${result.violations.join('\n')}`);
    process.exitCode = 1;
  } else
    console.log(
      `Product identifier guard OK: ${result.checked} files, ${result.terms} catalog terms, ${result.exceptions} exact exceptions.`,
    );
}
if (import.meta.main) {
  try {
    main();
  } catch (error) {
    console.error(`Product identifier guard: ${error.message}`);
    process.exitCode = 1;
  }
}
