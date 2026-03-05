import grpc from 'k6/net/grpc';
import { check, group } from 'k6';
import { options } from './helpers/config.js';
import { connect, close, client, SVC } from './helpers/grpc.js';
import { login, authMeta } from './helpers/auth.js';
import { randomName, sampleMapData } from './helpers/data.js';

export { options };

export function setup() {
  connect();
  const host = login(`room-host-${Date.now()}`);
  const player = login(`room-player-${Date.now()}`, 'room-player-salt', 'room-player-fp');

  // Create a map for room tests
  const mapResp = client.invoke(
    SVC.MAP.CREATE,
    { name: randomName('room-map'), data: sampleMapData(), description: 'Room test map' },
    authMeta(host.token),
  );

  close();
  return {
    hostToken: host.token,
    hostUserId: host.userId,
    playerToken: player.token,
    playerUserId: player.userId,
    mapId: mapResp.message.map.mapId,
  };
}

export default function (data) {
  connect();

  let roomId;

  group('CreateRoom - valid', () => {
    const resp = client.invoke(
      SVC.ROOM.CREATE,
      {
        map_id: data.mapId,
        max_players: 4,
        room_name: randomName('room'),
        is_public: true,
      },
      authMeta(data.hostToken),
    );
    check(resp, {
      'create room OK': (r) => r.status === grpc.StatusOK,
      'room has id': (r) => r.message.room.roomId.length > 0,
      'host is set': (r) => r.message.room.hostUserId === data.hostUserId,
      'state is waiting': (r) => r.message.room.state === 'ROOM_STATE_WAITING',
      'topology returned': (r) => r.message.topology !== null,
    });
    if (resp.status === grpc.StatusOK) {
      roomId = resp.message.room.roomId;
    }
  });

  group('GetRoom', () => {
    if (!roomId) return;
    const resp = client.invoke(SVC.ROOM.GET, { room_id: roomId }, authMeta(data.hostToken));
    check(resp, {
      'get room OK': (r) => r.status === grpc.StatusOK,
      'room id matches': (r) => r.message.room.roomId === roomId,
    });
  });

  group('GetRoom - non-existent', () => {
    const resp = client.invoke(
      SVC.ROOM.GET,
      { room_id: '00000000-0000-0000-0000-000000000000' },
      authMeta(data.hostToken),
    );
    check(resp, {
      'not found': (r) => r.status === grpc.StatusNotFound,
    });
  });

  group('ListRooms', () => {
    const resp = client.invoke(
      SVC.ROOM.LIST,
      { page_size: 10, states: ['ROOM_STATE_WAITING'] },
      authMeta(data.hostToken),
    );
    check(resp, {
      'list rooms OK': (r) => r.status === grpc.StatusOK,
      'rooms is array': (r) => Array.isArray(r.message.rooms),
    });
  });

  group('ListRooms - filter by map', () => {
    const resp = client.invoke(
      SVC.ROOM.LIST,
      { page_size: 10, map_id: data.mapId },
      authMeta(data.hostToken),
    );
    check(resp, {
      'filtered list OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('JoinRoom - as player', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.JOIN,
      { room_id: roomId, role: 'ROOM_ROLE_PARTICIPANT' },
      authMeta(data.playerToken),
    );
    check(resp, {
      'join room OK': (r) => r.status === grpc.StatusOK,
      'current players increased': (r) => r.message.room.currentPlayers >= 2,
    });
  });

  group('GetRoomUsers', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.GET_USERS,
      { room_id: roomId },
      authMeta(data.hostToken),
    );
    check(resp, {
      'get room users OK': (r) => r.status === grpc.StatusOK,
      'has multiple users': (r) => r.message.users.length >= 2,
    });
  });

  group('RegisterPeerId - host', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.REGISTER_PEER,
      { room_id: roomId, peer_id: `peer-host-${Date.now()}` },
      authMeta(data.hostToken),
    );
    check(resp, {
      'register peer OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('RegisterPeerId - player', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.REGISTER_PEER,
      { room_id: roomId, peer_id: `peer-player-${Date.now()}` },
      authMeta(data.playerToken),
    );
    check(resp, {
      'register player peer OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('ReportConnection', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.REPORT_CONNECTION,
      {
        room_id: roomId,
        peer_statuses: [
          { peer_id: `peer-player-${Date.now()}`, rtt_ms: 50, packet_loss: 0.01, connected: true },
        ],
      },
      authMeta(data.hostToken),
    );
    check(resp, {
      'report connection OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('GetTopology', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.GET_TOPOLOGY,
      { room_id: roomId },
      authMeta(data.hostToken),
    );
    check(resp, {
      'get topology OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('GetRoomTopology', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.GET_ROOM_TOPOLOGY,
      { room_id: roomId },
      authMeta(data.hostToken),
    );
    check(resp, {
      'get room topology OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('ResolvePeerIds', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.RESOLVE_PEERS,
      { room_id: roomId, peer_ids: [] },
      authMeta(data.hostToken),
    );
    check(resp, {
      'resolve peers OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('StartGame - by host', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.START,
      { room_id: roomId, start_frame: 0 },
      authMeta(data.hostToken),
    );
    check(resp, {
      'start game OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('ReportArrival', () => {
    if (!roomId) return;
    const resp = client.invoke(
      SVC.ROOM.REPORT_ARRIVAL,
      {
        room_id: roomId,
        arrived_user_id: data.playerUserId,
        arrival_frame: 100,
        rank: 1,
      },
      authMeta(data.hostToken),
    );
    check(resp, {
      'report arrival OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('KickPlayer - by non-host', () => {
    // Create a new room for kick test
    const createResp = client.invoke(
      SVC.ROOM.CREATE,
      { map_id: data.mapId, max_players: 4, room_name: randomName('kick-room'), is_public: true },
      authMeta(data.hostToken),
    );
    if (createResp.status !== grpc.StatusOK) return;
    const kickRoomId = createResp.message.room.roomId;

    client.invoke(SVC.ROOM.JOIN, { room_id: kickRoomId }, authMeta(data.playerToken));

    const resp = client.invoke(
      SVC.ROOM.KICK,
      { room_id: kickRoomId, target_user_id: data.hostUserId },
      authMeta(data.playerToken),
    );
    check(resp, {
      'non-host kick denied': (r) => r.status === grpc.StatusPermissionDenied,
    });
  });

  group('KickPlayer - by host', () => {
    const createResp = client.invoke(
      SVC.ROOM.CREATE,
      { map_id: data.mapId, max_players: 4, room_name: randomName('kick-room2'), is_public: true },
      authMeta(data.hostToken),
    );
    if (createResp.status !== grpc.StatusOK) return;
    const kickRoomId = createResp.message.room.roomId;

    client.invoke(SVC.ROOM.JOIN, { room_id: kickRoomId }, authMeta(data.playerToken));

    const resp = client.invoke(
      SVC.ROOM.KICK,
      { room_id: kickRoomId, target_user_id: data.playerUserId },
      authMeta(data.hostToken),
    );
    check(resp, {
      'host kick OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('StartGame - by non-host', () => {
    const createResp = client.invoke(
      SVC.ROOM.CREATE,
      { map_id: data.mapId, max_players: 4, room_name: randomName('start-room'), is_public: true },
      authMeta(data.hostToken),
    );
    if (createResp.status !== grpc.StatusOK) return;

    client.invoke(SVC.ROOM.JOIN, { room_id: createResp.message.room.roomId }, authMeta(data.playerToken));

    const resp = client.invoke(
      SVC.ROOM.START,
      { room_id: createResp.message.room.roomId, start_frame: 0 },
      authMeta(data.playerToken),
    );
    check(resp, {
      'non-host start denied': (r) => r.status === grpc.StatusPermissionDenied,
    });
  });

  close();
}

export function teardown(data) {
  connect();
  client.invoke(SVC.MAP.DELETE, { map_id: data.mapId }, authMeta(data.hostToken));
  close();
}
