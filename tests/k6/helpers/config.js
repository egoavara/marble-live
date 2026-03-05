export const GRPC_HOST = __ENV.K6_GRPC_HOST || 'localhost:3000';

export const MODES = {
  functional: {
    scenarios: {
      default: { executor: 'shared-iterations', iterations: 1, vus: 1 },
    },
    thresholds: { checks: ['rate==1.0'] },
  },
  load: {
    scenarios: {
      default: {
        executor: 'ramping-vus',
        startVUs: 1,
        stages: [
          { duration: '10s', target: parseInt(__ENV.K6_VUS || '10') },
          { duration: '30s', target: parseInt(__ENV.K6_VUS || '10') },
          { duration: '10s', target: 0 },
        ],
      },
    },
    thresholds: {
      checks: ['rate>0.95'],
      grpc_req_duration: ['p(95)<300'],
    },
  },
};

export const options = MODES[__ENV.K6_MODE || 'functional'];
