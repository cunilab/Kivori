#!/usr/bin/env bun
// Desktop builds include every catalog pair. Absolute manifests are created on the embedding host.
import { spawnSync } from 'node:child_process';
import {
  constants,
  copyFileSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  writeFileSync,
} from 'node:fs';
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import {
  buildFirmware,
  checkFirmwareBuildEnvironment,
  listFirmwarePlans,
} from './build-firmware.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const filename = (plan) => `firmware-${plan.productWire}-${plan.boardWire}.elf`;

function manifestLocation(directory, manifest) {
  if (!isAbsolute(directory) || !isAbsolute(manifest))
    throw new Error('Firmware directory and manifest output must be absolute paths.');
  if (/[\r\n]/.test(manifest)) throw new Error('Manifest output must fit on one stdout line.');
  const canonicalDirectory = join(realpathSync(dirname(directory)), basename(directory));
  const canonicalManifest = join(realpathSync(dirname(manifest)), basename(manifest));
  const rel = relative(canonicalDirectory, canonicalManifest);
  if (!rel || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith(`..${sep}`)))
    throw new Error('The manifest must be outside the ELF directory.');
  try {
    lstatSync(manifest);
    throw new Error('Manifest output already exists; use a fresh staging directory.');
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  if (process.env.KIVORI_FIRMWARE_PATH !== undefined)
    throw new Error('Unset KIVORI_FIRMWARE_PATH before preparing a complete firmware set.');
}

/** Validates transported filenames; native embedding checks their identities, versions and exact set. */
export function writeFirmwareManifest(root, directory, manifest) {
  manifestLocation(directory, manifest);
  if (!lstatSync(directory).isDirectory() || lstatSync(directory).isSymbolicLink())
    throw new Error('Firmware directory must be a real directory.');
  const canonicalDirectory = realpathSync(directory);
  const expected = listFirmwarePlans(root).map(filename);
  const actual = readdirSync(directory);
  if (actual.length !== expected.length || actual.some((file) => !expected.includes(file)))
    throw new Error(`Firmware directory must contain exactly: ${expected.join(', ')}`);
  const images = expected.map((file) => {
    const path = join(directory, file);
    const stat = lstatSync(path);
    if (!stat.isFile() || stat.size === 0)
      throw new Error(`Firmware image must be a nonempty regular file: ${file}`);
    const resolved = realpathSync(path);
    if (dirname(resolved) !== canonicalDirectory)
      throw new Error('Firmware image escaped its directory.');
    return resolved;
  });
  writeFileSync(manifest, `${JSON.stringify(images, null, 2)}\n`, { flag: 'wx' });
  return resolve(manifest);
}

/** Snapshots each successful build before another board can overwrite a shared Cargo artifact. */
export function buildFirmwareSet(root, directory, manifest, run = spawnSync) {
  manifestLocation(directory, manifest);
  const plans = listFirmwarePlans(root); // Validate the entire catalog before any build/output.
  checkFirmwareBuildEnvironment();
  mkdirSync(directory); // A fresh directory prevents stale files from satisfying completeness.
  for (const plan of plans) {
    const artifact = buildFirmware(plan, run);
    copyFileSync(artifact, join(directory, filename(plan)), constants.COPYFILE_EXCL);
  }
  return writeFirmwareManifest(root, directory, manifest);
}

if (import.meta.main) {
  try {
    const { values } = parseArgs({
      options: {
        targets: { type: 'boolean' },
        'out-dir': { type: 'string' },
        'from-dir': { type: 'string' },
        'manifest-out': { type: 'string' },
      },
    });
    const out = values['out-dir'];
    const from = values['from-dir'];
    const manifest = values['manifest-out'];
    if (values.targets) {
      if (out !== undefined || from !== undefined || manifest !== undefined)
        throw new Error('--targets cannot be combined with packaging paths.');
      console.log([...new Set(listFirmwarePlans(repo).map((p) => p.target))].sort().join('\n'));
    } else {
      if ((out === undefined) === (from === undefined) || !manifest)
        throw new Error('Use --out-dir OR --from-dir, with --manifest-out.');
      console.log(
        out !== undefined
          ? buildFirmwareSet(repo, out, manifest)
          : writeFirmwareManifest(repo, from, manifest),
      );
    }
  } catch (error) {
    console.error(`Firmware set: ${error.message}`);
    process.exitCode = 1;
  }
}
