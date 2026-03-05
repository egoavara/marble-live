import grpc from 'k6/net/grpc';
import { check, group } from 'k6';
import { options } from './helpers/config.js';
import { connect, close, client, SVC } from './helpers/grpc.js';
import { login, authMeta } from './helpers/auth.js';
import { randomColor } from './helpers/data.js';

export { options };

export function setup() {
  connect();
  const userA = login(`avatar-a-${Date.now()}`);
  const userB = login(`avatar-b-${Date.now()}`, 'avatar-b-salt', 'avatar-b-fp');
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

  group('SetAvatar - color type', () => {
    const resp = client.invoke(
      SVC.AVATAR.SET,
      {
        avatar_type: 'AVATAR_TYPE_COLOR',
        color: { fill_color: randomColor() },
        outline: { color: randomColor(), width: 2.0, style: 'OUTLINE_STYLE_SOLID' },
      },
      authMeta(data.tokenA),
    );
    check(resp, {
      'set color avatar OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('SetAvatar - image type', () => {
    const resp = client.invoke(
      SVC.AVATAR.SET,
      {
        avatar_type: 'AVATAR_TYPE_IMAGE',
        image: {
          image_url: 'https://example.com/avatar.png',
          crop_x: 0,
          crop_y: 0,
          crop_width: 128,
          crop_height: 128,
        },
        outline: { color: 0xFF0000FF, width: 1.5, style: 'OUTLINE_STYLE_DASHED' },
      },
      authMeta(data.tokenA),
    );
    check(resp, {
      'set image avatar OK': (r) => r.status === grpc.StatusOK,
    });
  });

  group('GetAvatar - own avatar', () => {
    const resp = client.invoke(
      SVC.AVATAR.GET,
      { user_id: data.userIdA },
      authMeta(data.tokenA),
    );
    check(resp, {
      'get avatar OK': (r) => r.status === grpc.StatusOK,
      'avatar user matches': (r) => r.message.avatar.userId === data.userIdA,
    });
  });

  group('GetAvatar - other user avatar', () => {
    // Set avatar for userB first
    client.invoke(
      SVC.AVATAR.SET,
      {
        avatar_type: 'AVATAR_TYPE_COLOR',
        color: { fill_color: 0x00FF00FF },
      },
      authMeta(data.tokenB),
    );

    const resp = client.invoke(
      SVC.AVATAR.GET,
      { user_id: data.userIdB },
      authMeta(data.tokenA),
    );
    check(resp, {
      'get other avatar OK': (r) => r.status === grpc.StatusOK,
      'other avatar user matches': (r) => r.message.avatar.userId === data.userIdB,
    });
  });

  group('GetAvatars - batch', () => {
    const resp = client.invoke(
      SVC.AVATAR.GET_MULTI,
      { user_ids: [data.userIdA, data.userIdB] },
      authMeta(data.tokenA),
    );
    check(resp, {
      'get avatars OK': (r) => r.status === grpc.StatusOK,
      'returns two avatars': (r) => r.message.avatars.length === 2,
    });
  });

  group('GetAvatars - empty list', () => {
    const resp = client.invoke(
      SVC.AVATAR.GET_MULTI,
      { user_ids: [] },
      authMeta(data.tokenA),
    );
    check(resp, {
      'empty request OK': (r) => r.status === grpc.StatusOK,
      'returns empty': (r) => r.message.avatars.length === 0,
    });
  });

  group('SetAvatar - update existing', () => {
    const newColor = randomColor();
    const resp = client.invoke(
      SVC.AVATAR.SET,
      {
        avatar_type: 'AVATAR_TYPE_COLOR',
        color: { fill_color: newColor },
        outline: { color: 0x000000FF, width: 3.0, style: 'OUTLINE_STYLE_DOTTED' },
      },
      authMeta(data.tokenA),
    );
    check(resp, {
      'update avatar OK': (r) => r.status === grpc.StatusOK,
    });

    const getResp = client.invoke(
      SVC.AVATAR.GET,
      { user_id: data.userIdA },
      authMeta(data.tokenA),
    );
    check(getResp, {
      'updated avatar reflects change': (r) => r.status === grpc.StatusOK,
    });
  });

  close();
}
