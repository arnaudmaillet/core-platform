use tonic::{Request, Status};
use crate::domain::value_objects::RegionCode;

/// Intercepteur gRPC pour extraire la région des métadonnées (Headers).
/// Injecté par le BFF ou la Gateway.
pub fn region_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    // 1. Extraction du header 'x-region'
    let region_raw = req.metadata()
        .get("x-region")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Status::invalid_argument("Missing 'x-region' header"))?;

    // 2. Conversion vers le Value Object du domaine (Validation)
    let region_code = RegionCode::try_from(region_raw.to_string())
        .map_err(|e| Status::invalid_argument(format!("Invalid region: {}", e)))?;

    // 3. Injection dans les extensions de la requête pour les handlers
    req.extensions_mut().insert(region_code);

    Ok(req)
}