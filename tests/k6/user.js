import grpc from 'k6/net/grpc';
import { check, group } from 'k6';
import { options } from './helpers/config.js';
import { connect, close, client, SVC } from './helpers/grpc.js';
import { login, authMeta } from './helpers/auth.js';
import { randomName } from './helpers/data.js';

export { options };

export function setup() {
  connect();
  const auth = login(`setup-user-${Date.now()}`);
  close();
  return { token: auth.token, userId: auth.userId };
}

export default function (data) {
  connect();

  group('Login - anonymous', () => {
    const resp = client.invoke(SVC.USER.LOGIN, {
      anonymous: {
        display_name: randomName('anon'),
        salt: `salt-${__VU}-${__ITER}-${Date.now()}`,
        fingerprint: `fp-${__VU}-${__ITER}-${Date.now()}`,
      },
    });
    check(resp, {
      'login status OK': (r) => r.status === grpc.StatusOK,
      'login returns token': (r) => r.message.token.length > 0,
      'login returns user': (r) => r.message.user.userId.length > 0,
      'login returns display_name': (r) => r.message.user.displayName.length > 0,
    });
  });

  group('Login - same credentials return same user', () => {
    const salt = `dedup-salt-${__VU}-${__ITER}`;
    const fp = `dedup-fp-${__VU}-${__ITER}`;
    const resp1 = client.invoke(SVC.USER.LOGIN, {
      anonymous: { display_name: 'dedup-user', salt: salt, fingerprint: fp },
    });
    const resp2 = client.invoke(SVC.USER.LOGIN, {
      anonymous: { display_name: 'dedup-user', salt: salt, fingerprint: fp },
    });
    check(resp1, { 'first login OK': (r) => r.status === grpc.StatusOK });
    check(resp2, { 'second login OK': (r) => r.status === grpc.StatusOK });
    check(resp2, {
      'same user returned': (r) => r.message.user.userId === resp1.message.user.userId,
    });
  });

  group('GetUser - existing user', () => {
    const resp = client.invoke(SVC.USER.GET_USER, { user_id: data.userId }, authMeta(data.token));
    check(resp, {
      'get user OK': (r) => r.status === grpc.StatusOK,
      'user id matches': (r) => r.message.user.userId === data.userId,
    });
  });

  group('GetUser - non-existent user', () => {
    const resp = client.invoke(
      SVC.USER.GET_USER,
      { user_id: '00000000-0000-0000-0000-000000000000' },
      authMeta(data.token),
    );
    check(resp, {
      'not found': (r) => r.status === grpc.StatusNotFound,
    });
  });

  group('GetUsers - batch', () => {
    const resp = client.invoke(SVC.USER.GET_USERS, { user_ids: [data.userId] }, authMeta(data.token));
    check(resp, {
      'get users OK': (r) => r.status === grpc.StatusOK,
      'returns one user': (r) => r.message.users.length === 1,
    });
  });

  group('GetUsers - empty list', () => {
    const resp = client.invoke(SVC.USER.GET_USERS, { user_ids: [] }, authMeta(data.token));
    check(resp, {
      'empty request OK': (r) => r.status === grpc.StatusOK,
      'returns empty': (r) => r.message.users.length === 0,
    });
  });

  group('UpdateProfile - valid', () => {
    const newName = randomName('updated');
    const resp = client.invoke(
      SVC.USER.UPDATE_PROFILE,
      { display_name: newName },
      authMeta(data.token),
    );
    check(resp, {
      'update profile OK': (r) => r.status === grpc.StatusOK,
      'name updated': (r) => r.message.user.displayName === newName,
    });
  });

  group('UpdateProfile - too long name', () => {
    const resp = client.invoke(
      SVC.USER.UPDATE_PROFILE,
      { display_name: 'a'.repeat(33) },
      authMeta(data.token),
    );
    check(resp, {
      'invalid argument': (r) => r.status === grpc.StatusInvalidArgument,
    });
  });

  group('UpdateProfile - empty name', () => {
    const resp = client.invoke(
      SVC.USER.UPDATE_PROFILE,
      { display_name: '' },
      authMeta(data.token),
    );
    check(resp, {
      'invalid argument for empty': (r) => r.status === grpc.StatusInvalidArgument,
    });
  });

  close();
}
