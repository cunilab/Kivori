#!/usr/bin/env bun
// One catalog-selected firmware build. Stdout is exclusively the artifact path for callers.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, realpathSync, statSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const machineId = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const cargoName = /^[a-zA-Z0-9_]+(?:[-/][a-zA-Z0-9_]+)*$/;
const toml = (path) => Bun.TOML.parse(readFileSync(path, 'utf8'));

function inside(parent, path) {
  if (
    typeof path !== 'string' ||
    !path ||
    isAbsolute(path) ||
    path.includes('\\') ||
    path.split('/').includes('..')
  ) {
    throw new Error('Catalog paths must be nonempty relative paths using forward slashes.');
  }
  const absolute = resolve(parent, path);
  const rel = relative(realpathSync(parent), realpathSync(absolute));
  if (
    !rel ||
    rel === '..' ||
    rel.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) ||
    isAbsolute(rel)
  ) {
    throw new Error(`Catalog path escapes its parent: ${path}`);
  }
  return absolute;
}

export function resolveFirmware(root, product, board) {
  if ((product === undefined) !== (board === undefined))
    throw new Error('Select both product and board.');
  if (product !== undefined && (!machineId.test(product) || !machineId.test(board))) {
    throw new Error('Product and board must be nonempty lowercase kebab-case ids.');
  }
  const registry = toml(join(root, 'products/registry.toml'));
  const pairs = [];
  for (const entry of readdirSync(join(root, 'products'), { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name.startsWith('.')) continue;
    if (!machineId.test(entry.name) || !registry.products?.[entry.name]) {
      throw new Error(`Unregistered product directory: ${entry.name}`);
    }
    const catalog = toml(join(root, 'products', entry.name, 'product.toml'));
    for (const [id, spec] of Object.entries(catalog.boards ?? {})) {
      if (!machineId.test(id) || !registry.boards?.[id])
        throw new Error(`Unregistered board: ${id}`);
      pairs.push({ product: entry.name, board: id, spec });
    }
  }
  const selected = product
    ? pairs.find((pair) => pair.product === product && pair.board === board)
    : pairs.length === 1
      ? pairs[0]
      : undefined;
  if (!selected)
    throw new Error(
      `Select a catalog product/board pair: ${pairs.map((p) => `${p.product}/${p.board}`).join(', ')}`,
    );
  const firmware = selected.spec.firmware;
  const build = firmware?.build;
  if (
    !build ||
    Object.keys(build).some((key) => !['target', 'features'].includes(key)) ||
    typeof build.target !== 'string' ||
    !/^[a-z0-9_]+(?:-[a-z0-9_]+)+$/.test(build.target) ||
    !Array.isArray(build.features) ||
    !build.features.every((f) => typeof f === 'string' && cargoName.test(f)) ||
    new Set(build.features).size !== build.features.length
  ) {
    throw new Error('Selected firmware needs explicit build.target and build.features.');
  }
  const crate = inside(root, firmware.crate);
  if (!existsSync(join(crate, 'Cargo.toml'))) throw new Error('Firmware crate has no Cargo.toml.');
  const prefix = `target/${build.target}/release/`;
  const binary =
    typeof firmware.artifact === 'string' && firmware.artifact.startsWith(prefix)
      ? firmware.artifact.slice(prefix.length)
      : '';
  if (!/^[a-zA-Z0-9_-]+$/.test(binary))
    throw new Error('Firmware artifact must be target/<target>/release/<binary>.');
  return {
    product: selected.product,
    board: selected.board,
    cwd: crate,
    artifact: join(crate, firmware.artifact),
    targetDir: join(crate, 'target'),
    args: [
      'build',
      '--locked',
      '--release',
      '--target',
      build.target,
      '--no-default-features',
      '--bin',
      binary,
      ...(build.features.length ? ['--features', build.features.join(',')] : []),
    ],
  };
}

export function buildFirmware(plan, run = spawnSync) {
  if (process.env.RUSTFLAGS !== undefined || process.env.CARGO_ENCODED_RUSTFLAGS !== undefined) {
    throw new Error(
      'Unset RUSTFLAGS and CARGO_ENCODED_RUSTFLAGS: firmware must retain its target linker flags.',
    );
  }
  const result = run('cargo', plan.args, {
    cwd: plan.cwd,
    env: { ...process.env, CARGO_TARGET_DIR: plan.targetDir },
    stdio: ['inherit', 2, 2],
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`Firmware build failed (${result.signal ?? result.status}).`);
  if (!existsSync(plan.artifact) || !statSync(plan.artifact).isFile())
    throw new Error(`Firmware artifact was not produced: ${plan.artifact}`);
  inside(plan.cwd, relative(plan.cwd, plan.artifact).split(sep).join('/'));
  return plan.artifact;
}

if (import.meta.main) {
  try {
    const { values } = parseArgs({
      options: {
        product: { type: 'string' },
        board: { type: 'string' },
      },
    });
    const fromCli = values.product !== undefined || values.board !== undefined;
    const plan = resolveFirmware(
      repo,
      fromCli ? values.product : process.env.KIVORI_PRODUCT,
      fromCli ? values.board : process.env.KIVORI_BOARD,
    );
    console.log(buildFirmware(plan));
  } catch (error) {
    console.error(`Firmware build: ${error.message}`);
    process.exitCode = 1;
  }
}
