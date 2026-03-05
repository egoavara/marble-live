import grpc from 'k6/net/grpc';
import { check } from 'k6';
import { client, SVC } from './grpc.js';

export function login(displayName, salt, fingerprint) {
  const resp = client.invoke(SVC.USER.LOGIN, {
    anonymous: {
      display_name: displayName || `k6-user-${Date.now()}`,
      salt: salt || `salt-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      fingerprint: fingerprint || `fp-${Date.now()}-${Math.random().toString(36).slice(2)}`,
    },
  });
  check(resp, { 'login OK': (r) => r.status === grpc.StatusOK });
  return {
    token: resp.message.token,
    userId: resp.message.user.userId,
    user: resp.message.user,
  };
}

export function authMeta(token) {
  return { metadata: { authorization: `Bearer ${token}` } };
}
