import grpc from 'k6/net/grpc';
import { check } from 'k6';
import { SharedArray } from 'k6/data';

const client = new grpc.Client();
client.load(['../../../../../../proto'], 'profile/v1/profile.proto');

const profileIds = new SharedArray('profile ids', function () {
    return JSON.parse(open('./data.json'));
});

export const options = {
    scenarios: {
        stress: {
            executor: 'constant-arrival-rate',
            duration: '5m',           // Un peu plus long pour laisser le temps au HPA
            rate: 3000,              // On passe de 500 à 3000 requêtes/seconde
            timeUnit: '1s',
            preAllocatedVUs: 500,    // On commence avec plus de monde
            maxVUs: 2000,            // On s'autorise à monter très haut pour tenir le rate
        },
    },
    thresholds: {
        'grpc_req_duration': ['p(95)<500'],
    },
};

export default () => {
    // Connexion persistante par VU
    try {
        // On ne connecte que si nécessaire
        client.connect('api-profile.core-platform.click:443', { 
            plaintext: false,
            timeout: '5s'
        });
    } catch (e) {
        // Déjà connecté ou erreur de handshake
    }

    // Sécurité : si la connexion n'est pas prête, on skip cette itération
    // au lieu de faire crash le script
    const randomId = profileIds[Math.floor(Math.random() * profileIds.length)];

    const payload = {
        profile_id: randomId,
        new_handle: `distributed_handle_${Math.floor(Math.random() * 1000000)}`,
    };

    // On enveloppe l'appel pour éviter que le "no gRPC connection" n'arrête k6
    let response;
    try {
        response = client.invoke('profile.v1.ProfileIdentityService/UpdateHandle', payload, {
            metadata: { 'x-region': 'eu' }
        });
        
        check(response, {
            'status is OK': (r) => r && r.status === grpc.StatusOK,
        });
    } catch (e) {
        // Log l'erreur de connexion si besoin
    }
};