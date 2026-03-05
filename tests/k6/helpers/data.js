export function randomName(prefix) {
  return `${prefix || 'test'}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function sampleMapData() {
  return JSON.stringify({
    objects: [{ type: 'floor', x: 0, y: -10, width: 20, height: 1 }],
    spawn: { x: 0, y: 5 },
    goals: [{ x: 0, y: -8 }],
  });
}

export function randomColor() {
  return Math.floor(Math.random() * 0xFFFFFFFF) >>> 0;
}
