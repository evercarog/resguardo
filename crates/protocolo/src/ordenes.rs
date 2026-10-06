//! Tipos de orden y qué autorización necesita cada uno (docs/api-servidor.md,
//! §1). Lo usan el servidor (para decidir qué puede enviar cada rol) y el
//! agente (para exigir el secreto que corresponde).

/// Qué hace falta para que el agente acepte una orden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nivel {
    /// Basta la sesión de la consola: no revela datos ni reduce la protección.
    Inofensiva,
    /// La contraseña del repositorio afectado.
    Repositorio,
    /// La clave de administración del cliente.
    Administracion,
    /// Las dos (p. ej. quitar un repositorio).
    RepositorioYAdministracion,
}

/// Un tipo de orden conocido.
#[derive(Clone, Copy, Debug)]
pub struct Tipo {
    pub nombre: &'static str,
    pub nivel: Nivel,
    /// Reduce la protección: el servidor exige `not_before ≥ ahora + espera` y el agente lo comprueba.
    pub destructiva: bool,
    /// Abre una sesión interactiva (`sesion` en los metadatos).
    pub abre_sesion: bool,
    /// Solo propietarios y administradores (no técnicos).
    pub solo_administradores: bool,
}

const fn t(nombre: &'static str, nivel: Nivel, destructiva: bool, abre_sesion: bool, solo_administradores: bool) -> Tipo {
    Tipo { nombre, nivel, destructiva, abre_sesion, solo_administradores }
}

use Nivel::*;

/// Todos los tipos de orden de la versión 2 del protocolo.
pub const TIPOS: &[Tipo] = &[
    // Inofensivas
    t("copiar_ahora", Inofensiva, false, false, false),
    t("verificar_ahora", Inofensiva, false, false, false),
    t("probar_restauracion", Inofensiva, false, false, false),
    t("subir_ahora", Inofensiva, false, false, false),
    t("desbloquear", Inofensiva, false, false, false),
    t("reanudar", Inofensiva, false, false, false),
    t("actualizar_agente", Inofensiva, false, false, false),
    t("abrir_sesion", Inofensiva, false, true, false),
    // v1.4x: cancelar una orden en espera en el equipo (de cualquiera de sus consolas).
    // Inofensiva: cancelar solo aumenta la protección (docs/consolas-multiples.md §5.7).
    t("cancelar_espera", Inofensiva, false, false, false),
    // Contraseña del repositorio
    t("explorar", Repositorio, false, true, false),
    t("restaurar", Repositorio, false, false, false),
    t("descargar", Repositorio, false, false, false),
    t("cambiar_retencion", Repositorio, true, false, false),
    t("aplicar_retencion", Repositorio, true, false, false),
    t("dejar_de_copiar", Repositorio, true, false, false),
    t("cambiar_copia_externa", Repositorio, false, false, false),
    // Tarea 4b: otras copias derivadas de un repositorio (docs/copias-en-cadena.md).
    // Cambiar una puede reducir la protección (según el cuerpo); quitarla, siempre.
    t("cambiar_derivada", Repositorio, false, false, false),
    t("quitar_derivada", Repositorio, true, false, false),
    t("rotar_contrasena_repo", Repositorio, false, false, false),
    t("quitar_repositorio", RepositorioYAdministracion, true, false, false),
    // Restaurar en otro equipo: el equipo de origen sella su acceso para el de destino.
    t("compartir_acceso", RepositorioYAdministracion, false, false, false),
    // v1.22: el equipo dueño añade al repositorio una clave de restic propia del
    // almacén donde está (para que el almacén aplique la retención en local).
    t("clave_almacen", RepositorioYAdministracion, false, false, false),
    // Clave de administración
    t("alta", Administracion, false, false, false),
    t("config", Administracion, false, false, false),
    t("elegir_carpetas", Administracion, false, true, false),
    t("crear_repositorio", Administracion, false, false, false),
    t("cambiar_destino", Administracion, false, false, false),
    t("importar_repositorio", Administracion, false, false, false),
    // v1.14: venir de la app de escritorio (adoptar un repositorio que ya existe y traer su historial).
    t("adoptar_repositorio", Administracion, false, false, false),
    t("copiar_historial", Administracion, false, false, false),
    t("servidores_respaldo", Administracion, false, false, true),
    // Nubes del espejo (Dropbox con OAuth PKCE): el permiso va sellado en la orden.
    t("conectar_nube", Administracion, false, false, false),
    t("quitar_nube", Administracion, false, false, false),
    t("pausar", Administracion, true, false, false),
    t("guarda_copias", Administracion, false, false, false),
    // v1.22: retención en el almacén (el equipo que guarda copias poda en local,
    // con su propia clave). Poner o cambiar la regla espera (según el cuerpo:
    // quitarla no); aplicarla ahora, siempre.
    t("retencion_almacen", Administracion, false, false, false),
    t("aplicar_retencion_almacen", Administracion, true, false, false),
    t("cambiar_espera", Administracion, false, false, false),
    t("baja_equipo", Administracion, true, false, true),
    t("desvincular", Administracion, false, false, true),
    t("cambiar_servidor", Administracion, false, false, true),
    t("cambiar_clave_admin", Administracion, false, false, true),
    // v1.35: varias consolas a la vez (docs/consolas-multiples.md). No reducen la
    // protección, pero son sensibles: solo administradores, y el equipo avisa a todas.
    t("anadir_consola", Administracion, false, false, true),
    t("quitar_consola", Administracion, false, false, true),
];

/// El tipo de orden por su nombre, o `None` si no se conoce (se rechaza).
pub fn tipo(nombre: &str) -> Option<&'static Tipo> {
    TIPOS.iter().find(|t| t.nombre == nombre)
}

/// Caducidad máxima de una orden.
pub const MAX_CADUCIDAD_DIAS: i64 = 7;

/// Variable de entorno de la prueba de extremo a extremo (`consola/scripts/e2e`):
/// con `1`, en una compilación de desarrollo, la espera mínima de las órdenes que
/// reducen la protección es de 0 s (el servidor y el agente la cumplen igual,
/// pero no hay que esperar horas). En la versión publicada (release) no existe.
pub const PRUEBA_SIN_ESPERA: &str = "RESGUARDO_PRUEBA_SIN_ESPERA";

/// Segundos que exige una espera mínima de `horas` (servidor y agente).
pub fn segundos_de_espera(horas: i64) -> i64 {
    if cfg!(debug_assertions) && std::env::var(PRUEBA_SIN_ESPERA).is_ok_and(|v| v == "1") {
        return 0;
    }
    horas * 3600
}

/// Tipos que reducen la protección solo con cierto cuerpo (api-servidor.md
/// §5). El servidor exige su espera cuando llevan `not_before`; el agente,
/// que ve el cuerpo, la exige siempre que el cuerpo sea destructivo.
/// `quitar_nube` lo es si el espejo usa esa nube (lo sabe el equipo).
/// `retencion_almacen` lo es salvo con `quitar: true` (deja de podar).
pub const DESTRUCTIVAS_SEGUN_CUERPO: &[&str] =
    &["desvincular", "guarda_copias", "cambiar_espera", "restaurar", "cambiar_copia_externa", "quitar_nube", "retencion_almacen", "cambiar_derivada"];
/// Tamaño máximo de un sobre de orden (base64).
pub const MAX_SOBRE_BYTES: usize = 64 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_espera_es_en_horas() {
        // Sin la variable de la prueba de extremo a extremo (nunca en la versión publicada).
        if std::env::var(PRUEBA_SIN_ESPERA).is_err() {
            assert_eq!(segundos_de_espera(1), 3600);
            assert_eq!(segundos_de_espera(24), 86_400);
        }
    }

    #[test]
    fn nombres_unicos_y_clasificacion() {
        let mut nombres: Vec<_> = TIPOS.iter().map(|t| t.nombre).collect();
        nombres.sort();
        let total = nombres.len();
        nombres.dedup();
        assert_eq!(total, nombres.len(), "tipos repetidos");
        // Pausar reduce la protección: nunca inofensiva.
        assert_eq!(tipo("pausar").unwrap().nivel, Nivel::Administracion);
        assert!(tipo("pausar").unwrap().destructiva);
        assert_eq!(tipo("copiar_ahora").unwrap().nivel, Nivel::Inofensiva);
        assert!(tipo("inventada").is_none());
        // Nubes: con la clave de administración; quitar una que usa el espejo espera (según el cuerpo).
        assert_eq!(tipo("conectar_nube").unwrap().nivel, Nivel::Administracion);
        assert_eq!(tipo("quitar_nube").unwrap().nivel, Nivel::Administracion);
        assert!(DESTRUCTIVAS_SEGUN_CUERPO.contains(&"quitar_nube") && !DESTRUCTIVAS_SEGUN_CUERPO.contains(&"conectar_nube"));
        // v1.22: retención en el almacén. La clave la añade el dueño (contraseña + administración);
        // ponerla espera según el cuerpo y aplicarla ahora siempre espera.
        assert_eq!(tipo("clave_almacen").unwrap().nivel, Nivel::RepositorioYAdministracion);
        assert_eq!(tipo("retencion_almacen").unwrap().nivel, Nivel::Administracion);
        assert!(DESTRUCTIVAS_SEGUN_CUERPO.contains(&"retencion_almacen") && !tipo("retencion_almacen").unwrap().destructiva);
        assert!(tipo("aplicar_retencion_almacen").unwrap().destructiva);
        // v1.35: varias consolas. Con la clave de administración, solo administradores, sin espera.
        for t in ["anadir_consola", "quitar_consola"] {
            let t = tipo(t).unwrap();
            assert!(t.nivel == Nivel::Administracion && t.solo_administradores && !t.destructiva);
        }
        // v1.4x: cancelar una orden en espera, desde cualquier consola y sin clave.
        let c = tipo("cancelar_espera").unwrap();
        assert!(c.nivel == Nivel::Inofensiva && !c.destructiva && !c.solo_administradores);
        // Ninguna inofensiva es destructiva.
        assert!(TIPOS.iter().filter(|t| t.nivel == Nivel::Inofensiva).all(|t| !t.destructiva));
    }
}
