import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';
import { SharedArray } from 'k6/data';

const client = new grpc.Client();
client.load(['../../../../../../proto'], 'profile/v1/profile.proto');

// Chargement des IDs depuis le fichier JSON
const profileIds = new SharedArray('profile ids', function () {
    return JSON.parse(open('./data.json'));
});

export const options = {
    stages: [
        { duration: '30s', target: 50 },  // Montée plus rapide
        { duration: '1m', target: 200 }, // On double la mise à 200 VUs
        { duration: '30s', target: 0 },
    ],
    thresholds: {
        'grpc_req_duration': ['p(95)<500'], // On devient plus exigeant (500ms)
    },
};

export default () => {
    client.connect('localhost:50051', { plaintext: true });

    // On pioche un ID au hasard dans la liste
    const randomId = profileIds[Math.floor(Math.random() * profileIds.length)];

    const payload = {
        profile_id: randomId,
        new_handle: `distributed_handle_${Math.floor(Math.random() * 1000000)}`,
    };

    const response = client.invoke('profile.v1.ProfileIdentityService/UpdateHandle', payload, {
        metadata: { 'x-region': 'eu' }
    });

    check(response, {
        'status is OK': (r) => r && (
            r.status === 0 || 
            r.status === "0" || 
            r.status === "OK" || 
            String(r.status).toLowerCase() === "ok"
        ),
    });

    client.close();
    sleep(0.1);
};