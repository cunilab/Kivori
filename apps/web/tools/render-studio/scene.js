// Browser side of the studio render tool (loaded by scene.html, driven by render.mjs through
// `window.shot(config)`). Plain JS on purpose: it runs in headless Chrome, never in the Next bundle.
import * as THREE from 'three';
import { STLLoader } from 'three/addons/loaders/STLLoader.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';
import { toCreasedNormals } from 'three/addons/utils/BufferGeometryUtils.js';

// ---- Assembly -------------------------------------------------------------------------------
// The STLs are in print orientation (see hardware/enclosure/build_case.py `print_pose`): each part is
// rotated for the print bed and translated so its bounding box starts at the origin. These matrices
// undo that and return every part to the CAD "face frame" (face plane z = 0, +z towards the viewer,
// x right, y from the near/thin edge at 0 to the far/thick edge at 70). The offsets are the original
// bounding-box minimums printed by build_case.py (see README.md in this folder).
const ALPHA = Math.atan((30 - 18) / 70); // wedge angle: the face tilts towards the viewer
const T_NEAR = 18;
const PLINTH_H = 3; // the plinth hangs this far below the back plane; that is the desk

const m4 = (...e) => new THREE.Matrix4().set(...e);
// Rotation by 180 degrees about X (y -> -y, z -> -z) followed by a translation.
const flipX = (tx, ty, tz) => m4(1, 0, 0, tx, 0, -1, 0, ty, 0, 0, -1, tz, 0, 0, 0, 1);
const move = (x, y, z) => new THREE.Matrix4().makeTranslation(x, y, z);

const PARTS = {
  Shell: { matrix: flipX(-3.01077, 73.21098, 0), mat: 'shell' },
  Visor: { matrix: flipX(44, 62.5, 1), mat: 'visor' },
  // Printed rotated by +ALPHA about X: undo with the inverse rotation after restoring its bbox minimum.
  BackPlate: {
    matrix: new THREE.Matrix4().makeRotationX(-ALPHA).multiply(move(-0.81041, 3.22001, -20.7412)),
    mat: 'visor',
  },
  Knob: { matrix: flipX(3.5, 59, 13), mat: 'knob' },
  CapLeft: { matrix: move(51.18391, 7.13261, -4), mat: 'cap' },
  CapMiddle: { matrix: move(70.4, 10.4, -4), mat: 'cap' },
  CapRight: { matrix: move(87.18391, 7.13261, -4), mat: 'cap' },
};

// Face frame -> three.js world: lift by the thin-edge thickness, tilt by the wedge angle so the back
// plane lies flat on the desk, then turn the CAD z-up frame into three's y-up frame (CAD y = depth away
// from the viewer). Centred on x = 55 and y = 35, floor at y = 0.
const FACE_TO_WORLD = move(-55, 0, 35)
  .multiply(new THREE.Matrix4().makeRotationX(-Math.PI / 2))
  .multiply(move(0, 0, PLINTH_H))
  .multiply(new THREE.Matrix4().makeRotationX(ALPHA))
  .multiply(move(0, 0, T_NEAR));

// Display window centre and active-area size on the face (face frame, mm).
const DISPLAY = { x: 78, y: 46, size: 23.4, z: 0.25 };

// ---- Renderer / scene -----------------------------------------------------------------------
const renderer = new THREE.WebGLRenderer({
  antialias: true,
  alpha: true,
  preserveDrawingBuffer: true,
});
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.0;
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.shadowMap.enabled = true;
renderer.shadowMap.type = THREE.VSMShadowMap;
document.body.appendChild(renderer.domElement);

const scene = new THREE.Scene();
const pmrem = new THREE.PMREMGenerator(renderer);
scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
scene.environmentIntensity = 0.75;

const MATERIALS = {
  shell: new THREE.MeshPhysicalMaterial({
    color: '#ebe6dd',
    roughness: 0.62,
    metalness: 0,
    sheen: 0.25,
    sheenRoughness: 0.8,
    sheenColor: '#ffffff',
  }),
  visor: new THREE.MeshPhysicalMaterial({
    color: '#0d0e10',
    roughness: 0.32,
    metalness: 0,
    clearcoat: 0.35,
    clearcoatRoughness: 0.3,
  }),
  knob: new THREE.MeshPhysicalMaterial({
    color: '#141518',
    roughness: 0.38,
    metalness: 0.1,
    clearcoat: 0.2,
    clearcoatRoughness: 0.4,
  }),
  cap: new THREE.MeshPhysicalMaterial({
    color: '#3fd09c',
    roughness: 0.55,
    metalness: 0,
    clearcoat: 0.15,
    clearcoatRoughness: 0.5,
  }),
};

// ---- Lights ---------------------------------------------------------------------------------
const key = new THREE.DirectionalLight('#fff6ea', 2.2);
key.position.set(-120, 220, 160);
key.castShadow = true;
key.shadow.mapSize.set(4096, 4096);
key.shadow.camera.left = -140;
key.shadow.camera.right = 140;
key.shadow.camera.top = 140;
key.shadow.camera.bottom = -140;
key.shadow.camera.near = 50;
key.shadow.camera.far = 600;
key.shadow.radius = 14;
key.shadow.blurSamples = 25;
key.shadow.bias = -0.0004;
scene.add(key);
const fill = new THREE.DirectionalLight('#eaf2ff', 0.7);
fill.position.set(200, 90, 120);
scene.add(fill);

// ---- Floor: soft shadow only, so the backdrop stays seamless --------------------------------
const floor = new THREE.Mesh(
  new THREE.PlaneGeometry(280, 280),
  new THREE.ShadowMaterial({ opacity: 0.2 }),
);
floor.rotation.x = -Math.PI / 2;
floor.receiveShadow = true;
scene.add(floor);

// Baked contact shadow: a blurred rounded rectangle just under the plinth, so the device is grounded
// even where the directional shadow is faint.
function contactShadow() {
  const c = document.createElement('canvas');
  c.width = c.height = 512;
  const g = c.getContext('2d');
  g.filter = 'blur(22px)';
  g.fillStyle = 'rgba(0,0,0,0.85)';
  g.beginPath();
  g.roundRect(150, 170, 212, 172, 30);
  g.fill();
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  const mesh = new THREE.Mesh(
    new THREE.PlaneGeometry(300, 240),
    new THREE.MeshBasicMaterial({
      map: tex,
      transparent: true,
      opacity: 0.55,
      depthWrite: false,
      toneMapped: false,
    }),
  );
  mesh.rotation.x = -Math.PI / 2;
  mesh.position.y = 0.02;
  return mesh;
}
scene.add(contactShadow());

// ---- Device ---------------------------------------------------------------------------------
const device = new THREE.Group();
scene.add(device);

const loader = new STLLoader();
await Promise.all(
  Object.entries(PARTS).map(async ([name, part]) => {
    let geo = await loader.loadAsync(`/stl/${name}`);
    geo.applyMatrix4(part.matrix);
    geo.applyMatrix4(FACE_TO_WORLD);
    geo = toCreasedNormals(geo, THREE.MathUtils.degToRad(35));
    const mesh = new THREE.Mesh(geo, MATERIALS[part.mat]);
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    device.add(mesh);
  }),
);

// Screen: unlit so it glows, with a black backing so a texture edge never shows the cavity.
const screenMat = new THREE.MeshBasicMaterial({ color: '#ffffff', toneMapped: false });
const screen = new THREE.Mesh(new THREE.PlaneGeometry(DISPLAY.size, DISPLAY.size), screenMat);
const backing = new THREE.Mesh(
  new THREE.PlaneGeometry(DISPLAY.size + 1.2, DISPLAY.size + 1.2),
  new THREE.MeshBasicMaterial({ color: '#05070c', toneMapped: false }),
);
for (const [mesh, dz] of [
  [backing, -0.02],
  [screen, 0],
]) {
  mesh.position.set(DISPLAY.x, DISPLAY.y, DISPLAY.z + dz);
  mesh.applyMatrix4(FACE_TO_WORLD);
  device.add(mesh);
}

const textures = new Map();
const texLoader = new THREE.TextureLoader();
async function setScreen(name) {
  if (!textures.has(name)) {
    const t = await texLoader.loadAsync(`/screens/${name}.webp`);
    t.colorSpace = THREE.SRGBColorSpace;
    t.anisotropy = renderer.capabilities.getMaxAnisotropy();
    textures.set(name, t);
  }
  screenMat.map = textures.get(name);
  screenMat.needsUpdate = true;
}

// ---- Shots ----------------------------------------------------------------------------------
// cfg: { width, height, background: '#rrggbb' | null, screen, fov, target: [x,y,z], azimuth, elevation,
//        distance } angles in degrees; azimuth 0 = straight in front, positive = camera to the right.
window.shot = async (cfg) => {
  await setScreen(cfg.screen);
  floor.visible = cfg.floorShadow !== false;
  renderer.setPixelRatio(window.devicePixelRatio);
  renderer.setSize(cfg.width, cfg.height);
  scene.background = cfg.background ? new THREE.Color(cfg.background) : null;
  renderer.setClearColor(0x000000, cfg.background ? 1 : 0);
  const cam = new THREE.PerspectiveCamera(cfg.fov, cfg.width / cfg.height, 5, 2000);
  const az = THREE.MathUtils.degToRad(cfg.azimuth);
  const el = THREE.MathUtils.degToRad(cfg.elevation);
  const t = new THREE.Vector3(...cfg.target);
  cam.position.set(
    t.x + cfg.distance * Math.cos(el) * Math.sin(az),
    t.y + cfg.distance * Math.sin(el),
    t.z + cfg.distance * Math.cos(el) * Math.cos(az),
  );
  cam.up.set(0, 1, 0);
  if (cfg.elevation > 85) cam.up.set(0, 0, -1);
  cam.lookAt(t);
  renderer.render(scene, cam);
  return true;
};
window.studioReady = true;
