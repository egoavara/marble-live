import grpc from 'k6/net/grpc';
import { check, group } from 'k6';
import { options } from './helpers/config.js';
import { connect, close, client, SVC } from './helpers/grpc.js';
import { login, authMeta } from './helpers/auth.js';
import { randomName, sampleMapData } from './helpers/data.js';

export { options };

export function setup() {
  connect();
  const userA = login(`map-owner-${Date.now()}`);
  const userB = login(`map-other-${Date.now()}`, 'map-other-salt', 'map-other-fp');
  close();
  return {
    tokenA: userA.token,
    userIdA: userA.userId,
    tokenB: userB.token,
    userIdB: userB.userId,
  };
}

export default function (data) {
  connect();

  let mapId;

  group('CreateMap - valid', () => {
    const resp = client.invoke(
      SVC.MAP.CREATE,
      {
        name: randomName('map'),
        data: sampleMapData(),
        description: 'Test map for k6',
        tags: ['test', 'k6'],
      },
      authMeta(data.tokenA),
    );
    check(resp, {
      'create map OK': (r) => r.status === grpc.StatusOK,
      'map has id': (r) => r.message.map.mapId.length > 0,
      'map has name': (r) => r.message.map.name.length > 0,
      'creator matches': (r) => r.message.map.creatorId === data.userIdA,
    });
    if (resp.status === grpc.StatusOK) {
      mapId = resp.message.map.mapId;
    }
  });

  group('CreateMap - invalid name (empty)', () => {
    const resp = client.invoke(
      SVC.MAP.CREATE,
      { name: '', data: sampleMapData() },
      authMeta(data.tokenA),
    );
    check(resp, {
      'empty name rejected': (r) => r.status === grpc.StatusInvalidArgument,
    });
  });

  group('CreateMap - invalid name (too long)', () => {
    const resp = client.invoke(
      SVC.MAP.CREATE,
      { name: 'a'.repeat(65), data: sampleMapData() },
      authMeta(data.tokenA),
    );
    check(resp, {
      'long name rejected': (r) => r.status === grpc.StatusInvalidArgument,
    });
  });

  group('GetMap - existing', () => {
    if (!mapId) return;
    const resp = client.invoke(SVC.MAP.GET, { map_id: mapId }, authMeta(data.tokenA));
    check(resp, {
      'get map OK': (r) => r.status === grpc.StatusOK,
      'map id matches': (r) => r.message.map.mapId === mapId,
    });
  });

  group('GetMap - non-existent', () => {
    const resp = client.invoke(
      SVC.MAP.GET,
      { map_id: '00000000-0000-0000-0000-000000000000' },
      authMeta(data.tokenA),
    );
    check(resp, {
      'not found': (r) => r.status === grpc.StatusNotFound,
    });
  });

  group('UpdateMap - by owner', () => {
    if (!mapId) return;
    const newName = randomName('updated-map');
    const resp = client.invoke(
      SVC.MAP.UPDATE,
      {
        map_id: mapId,
        name: newName,
        description: 'Updated description',
        tags: ['updated'],
        update_tags: true,
      },
      authMeta(data.tokenA),
    );
    check(resp, {
      'update map OK': (r) => r.status === grpc.StatusOK,
      'name updated': (r) => r.message.map.name === newName,
    });
  });

  group('UpdateMap - by non-owner', () => {
    if (!mapId) return;
    const resp = client.invoke(
      SVC.MAP.UPDATE,
      { map_id: mapId, name: 'hijacked' },
      authMeta(data.tokenB),
    );
    check(resp, {
      'non-owner denied': (r) =>
        r.status === grpc.StatusPermissionDenied || r.status === grpc.StatusNotFound,
    });
  });

  group('ListMaps - basic', () => {
    const resp = client.invoke(
      SVC.MAP.LIST,
      { page_size: 10 },
      authMeta(data.tokenA),
    );
    check(resp, {
      'list maps OK': (r) => r.status === grpc.StatusOK,
      'maps is array': (r) => Array.isArray(r.message.maps),
    });
  });

  group('ListMaps - filter by creator', () => {
    const resp = client.invoke(
      SVC.MAP.LIST,
      { page_size: 10, creator_id: data.userIdA },
      authMeta(data.tokenA),
    );
    check(resp, {
      'filtered list OK': (r) => r.status === grpc.StatusOK,
      'all maps by creator': (r) =>
        r.message.maps.every((m) => m.creatorId === data.userIdA),
    });
  });

  group('ListMaps - pagination', () => {
    const resp = client.invoke(
      SVC.MAP.LIST,
      { page_size: 1 },
      authMeta(data.tokenA),
    );
    check(resp, {
      'pagination OK': (r) => r.status === grpc.StatusOK,
      'respects page_size': (r) => r.message.maps.length <= 1,
    });
  });

  group('DeleteMap - by owner', () => {
    if (!mapId) return;
    const resp = client.invoke(
      SVC.MAP.DELETE,
      { map_id: mapId },
      authMeta(data.tokenA),
    );
    check(resp, {
      'delete map OK': (r) => r.status === grpc.StatusOK,
    });

    const getResp = client.invoke(SVC.MAP.GET, { map_id: mapId }, authMeta(data.tokenA));
    check(getResp, {
      'deleted map not found': (r) => r.status === grpc.StatusNotFound,
    });
  });

  group('DeleteMap - by non-owner', () => {
    // Create a map to attempt deletion by non-owner
    const createResp = client.invoke(
      SVC.MAP.CREATE,
      { name: randomName('nodelete'), data: sampleMapData() },
      authMeta(data.tokenA),
    );
    if (createResp.status !== grpc.StatusOK) return;
    const tempMapId = createResp.message.map.mapId;

    const resp = client.invoke(
      SVC.MAP.DELETE,
      { map_id: tempMapId },
      authMeta(data.tokenB),
    );
    check(resp, {
      'non-owner delete denied': (r) =>
        r.status === grpc.StatusPermissionDenied || r.status === grpc.StatusNotFound,
    });

    // Cleanup
    client.invoke(SVC.MAP.DELETE, { map_id: tempMapId }, authMeta(data.tokenA));
  });

  close();
}
