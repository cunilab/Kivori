// Release tooling uses the same active pairs and contained crate paths as packaging. Run with Bun.
import { lstatSync, readFileSync, realpathSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { listFirmwarePlans } from './build-firmware.mjs';

export function releaseCatalog(root) {
  const products = new Map();
  for (const plan of listFirmwarePlans(root)) {
    let product = products.get(plan.product);
    if (!product) {
      const names = Bun.TOML.parse(
        readFileSync(join(root, 'products', plan.product, 'product.toml'), 'utf8'),
      );
      for (const key of ['short_name', 'display_name']) {
        if (typeof names[key] !== 'string' || !names[key].trim() || /[\r\n]/.test(names[key]))
          throw new Error(`Invalid ${key} for ${plan.product}`);
      }
      product = {
        product: plan.product,
        name: names.short_name.trim(),
        display: names.display_name.trim(),
        crates: [],
      };
      products.set(plan.product, product);
    }
    const crate = relative(realpathSync(root), realpathSync(plan.cwd)).split(sep).join('/');
    if (!product.crates.includes(crate)) product.crates.push(crate);
  }
  for (const product of products.values()) {
    const versions = product.crates.map((crate) => {
      const path = join(root, crate, 'Cargo.toml');
      if (!lstatSync(path).isFile())
        throw new Error(`Firmware manifest must be a regular file: ${path}`);
      const manifest = Bun.TOML.parse(readFileSync(path, 'utf8'));
      if (!manifest.workspace || typeof manifest.workspace !== 'object')
        throw new Error(`Firmware crate must be an explicit workspace root: ${crate}`);
      const version = manifest.package?.version;
      if (
        typeof version !== 'string' ||
        !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)
      )
        throw new Error(`Explicit X.Y.Z package version required in ${crate}/Cargo.toml`);
      return version;
    });
    if (new Set(versions).size !== 1)
      throw new Error(`All boards of ${product.product} must have the same firmware version`);
    product.version = versions[0];
  }
  return [...products.values()];
}
