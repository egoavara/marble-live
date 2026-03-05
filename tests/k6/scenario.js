import grpc from 'k6/net/grpc';
import { check, group } from 'k6';
import { options } from './helpers/config.js';
import { connect, close, client, SVC } from './helpers/grpc.js';
import { authMeta } from './helpers/auth.js';
import { randomName, sampleMapData, randomColor } from './helpers/data.js';

export { options };

export default function () {
  connect();

  let tokenA, userIdA, tokenB, userIdB;
  let mapId, roomId;

  group('Step 1: Login user A', () => {
    const resp = client.invoke(SVC.USER.LOGIN, {
      anonymous: {
        display_name: randomName('scenario-a'),
        salt: `scenario-a-${__VU}-${__ITER}-${Date.now()}`,
        fingerprint: `scenario-a-fp-${__VU}-${__ITER}`,
      },
    });
    check(resp, { 'login A OK': (r) => r.status === grpc.StatusOK });
    if (resp.status === grpc.StatusOK) {
      tokenA = resp.message.token;
      userIdA = resp.message.user.userId;
    }
  });

  group('Step 2: Login user B', () => {
    const resp = client.invoke(SVC.USER.LOGIN, {
      anonymous: {
        display_name: randomName('scenario-b'),
        salt: `scenario-b-${__VU}-${__ITER}-${Date.now()}`,
        fingerprint: `scenario-b-fp-${__VU}-${__ITER}`,
      },
    });
    check(resp, { 'login B OK': (r) => r.status === grpc.StatusOK });
    if (resp.status === grpc.StatusOK) {
      tokenB = resp.message.token;
      userIdB = resp.message.user.userId;
    }
  });

  if (!tokenA || !tokenB) {
    close();
    return;
  }

  group('Step 3: Create map', () => {
    const resp = client.invoke(
      SVC.MAP.CREATE,
      {
        name: randomName('scenario-map'),
        data: sampleMapData(),
        description: 'Integration test map',
        tags: ['scenario'],
      },
      authMeta(tokenA),
    );
    check(resp, { 'create map OK': (r) => r.status === grpc.StatusOK });
    if (resp.status === grpc.StatusOK) {
      mapId = resp.message.map.mapId;
    }
  });

  if (!mapId) {
    close();
    return;
  }

  group('Step 4: Create room', () => {
    const resp = client.invoke(
      SVC.ROOM.CREATE,
      {
        map_id: mapId,
        max_players: 4,
        room_name: randomName('scenario-room'),
        is_public: true,
      },
      authMeta(tokenA),
    );
    check(resp, {
      'create room OK': (r) => r.status === grpc.StatusOK,
      'host is user A': (r) => r.message.room.hostUserId === userIdA,
    });
    if (resp.status === grpc.StatusOK) {
      roomId = resp.message.room.roomId;
    }
  });

  if (!roomId) {
    client.invoke(SVC.MAP.DELETE, { map_id: mapId }, authMeta(tokenA));
    close();
    return;
  }

  group('Step 5: Set avatars', () => {
    const respA = client.invoke(
      SVC.AVATAR.SET,
      {
        avatar_type: 'AVATAR_TYPE_COLOR',
        color: { fill_color: randomColor() },
        outline: { color: 0x000000FF, width: 2.0, style: 'OUTLINE_STYLE_SOLID' },
      },
      authMeta(tokenA),
    );
    check(respA, { 'set avatar A OK': (r) => r.status === grpc.StatusOK });

    const respB = client.invoke(
      SVC.AVATAR.SET,
      {
        avatar_type: 'AVATAR_TYPE_COLOR',
        color: { fill_color: randomColor() },
        outline: { color: 0xFFFFFFFF, width: 1.0, style: 'OUTLINE_STYLE_DASHED' },
      },
      authMeta(tokenB),
    );
    check(respB, { 'set avatar B OK': (r) => r.status === grpc.StatusOK });
  });

  group('Step 6: User B joins room', () => {
    const resp = client.invoke(
      SVC.ROOM.JOIN,
      { room_id: roomId, role: 'ROOM_ROLE_PARTICIPANT' },
      authMeta(tokenB),
    );
    check(resp, {
      'join room OK': (r) => r.status === grpc.StatusOK,
      'player count >= 2': (r) => r.message.room.currentPlayers >= 2,
    });
  });

  group('Step 7: Register peer IDs', () => {
    const hostPeer = `peer-host-${Date.now()}`;
    const playerPeer = `peer-player-${Date.now()}`;

    const respA = client.invoke(
      SVC.ROOM.REGISTER_PEER,
      { room_id: roomId, peer_id: hostPeer },
      authMeta(tokenA),
    );
    check(respA, { 'register host peer OK': (r) => r.status === grpc.StatusOK });

    const respB = client.invoke(
      SVC.ROOM.REGISTER_PEER,
      { room_id: roomId, peer_id: playerPeer },
      authMeta(tokenB),
    );
    check(respB, { 'register player peer OK': (r) => r.status === grpc.StatusOK });
  });

  group('Step 8: Start game', () => {
    const resp = client.invoke(
      SVC.ROOM.START,
      { room_id: roomId, start_frame: 0 },
      authMeta(tokenA),
    );
    check(resp, { 'start game OK': (r) => r.status === grpc.StatusOK });
  });

  group('Step 9: Report arrival', () => {
    const resp = client.invoke(
      SVC.ROOM.REPORT_ARRIVAL,
      {
        room_id: roomId,
        arrived_user_id: userIdB,
        arrival_frame: 200,
        rank: 1,
      },
      authMeta(tokenA),
    );
    check(resp, { 'report arrival OK': (r) => r.status === grpc.StatusOK });
  });

  group('Step 10: Get avatars for both users', () => {
    const resp = client.invoke(
      SVC.AVATAR.GET_MULTI,
      { user_ids: [userIdA, userIdB] },
      authMeta(tokenA),
    );
    check(resp, {
      'get avatars OK': (r) => r.status === grpc.StatusOK,
      'both avatars returned': (r) => r.message.avatars.length === 2,
    });
  });

  group('Step 11: Cleanup - delete map (may fail if room active)', () => {
    const resp = client.invoke(SVC.MAP.DELETE, { map_id: mapId }, authMeta(tokenA));
    check(resp, {
      'delete map responded': (r) =>
        r.status === grpc.StatusOK || r.status === grpc.StatusFailedPrecondition,
    });
  });

  close();
}
