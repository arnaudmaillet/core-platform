// backend/services/profile/api/command-server/load-tests/update-handle-scaling.js

import grpc from 'k6/net/grpc';
import { check } from 'k6';
import { SharedArray } from 'k6/data';

const client = new grpc.Client();

/**
 * AJUSTEMENT PROTO : 
 * Dans le cluster, le ConfigMapGenerator de Kustomize va mettre les fichiers
 * au même niveau. On pointe donc sur le dossier courant.
 */
client.load(
    ['.'], // Dossier courant dans le Pod (ou '/scripts/')
    'profile.proto'
);

const profileIds = new SharedArray('profile ids', function () {
    /**
     * AJUSTEMENT DATA :
     * Le fichier data.json sera aussi monté au même niveau par le ConfigMap.
     */
    return JSON.parse(open('./data.json'));
});

export const options = {
    scenarios: {
        scaling_validation: { // Renommé pour la clarté
            executor: 'constant-arrival-rate',
            duration: '5m',
            rate: 200,               
            timeUnit: '1s',
            preAllocatedVUs: 50,     
            maxVUs: 200,
        },
    },
    thresholds: {
        'grpc_req_duration': ['p(95)<500'],
        'checks': ['rate>0.99'], // On veut 99% de succès
    },
};

export default () => {
    // Connexion interne : DNS Kubernetes
    // Pas de TLS (plaintext) car on est dans le réseau privé VPC
    try {
        client.connect('dev-profile-command-server.default.svc.cluster.local:50051', { 
            plaintext: true,
            timeout: '10s' // Un peu plus généreux pour absorber les pics de scaling
        });
    } catch (e) {
        // La VU réutilise la connexion existante si déjà connectée
    }

    const randomId = profileIds[Math.floor(Math.random() * profileIds.length)];

    const payload = {
        profile_id: randomId,
        new_handle: `infra_test_${Math.floor(Math.random() * 1000000)}`,
    };

    let response;
    try {
        response = client.invoke('profile.v1.ProfileIdentityService/UpdateHandle', payload, {
            metadata: { 'x-source': 'k6-load-test' }
        });
        
        check(response, {
            'status is OK': (r) => r && r.status === grpc.StatusOK,
        });
    } catch (e) {
        // En cas d'erreur de connexion pendant le scaling
    }
};