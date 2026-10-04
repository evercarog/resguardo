//! PostgreSQL (pendiente). El diseño (docs/plataforma.md, §2.3): un esquema
//! por cliente más un esquema `control`, roles de base de datos por conexión
//! y RLS como defensa adicional, implementando el mismo trait
//! [`super::Almacen`] que SQLite. Las pruebas de la API se ejecutarán contra
//! los dos motores.

/// Conectar con PostgreSQL. Aún no está disponible: usa SQLite (por defecto).
pub fn conectar(_url: &str) -> Result<std::sync::Arc<dyn super::Almacen>, String> {
    Err("PostgreSQL aún no está disponible en esta versión de Resguardo Server: usa SQLite (por defecto).".into())
}
