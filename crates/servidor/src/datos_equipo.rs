//! v1.56: el nombre, las etiquetas y la observación que tiene el propio equipo
//! (docs/consolas-multiples.md §6).
//!
//! Un agente que lo admite (`admite: "datos_equipo"`) guarda el valor canónico de
//! estos datos (puesto con las órdenes `nombre_equipo`, `etiquetas_equipo` y
//! `observacion_equipo` desde cualquiera de sus consolas) y lo dice en su resumen
//! (`datos_equipo`). Cada servidor lo copia a lo suyo al recibir el resumen: así
//! todas las consolas enseñan lo mismo, y también los avisos, los correos y los
//! informes de este servidor.
//!
//! Solo lo que el equipo dice que tiene puesto: un dato que no está en el resumen
//! (agente anterior, o nadie lo cambió todavía con su orden) no toca lo de aquí.
//! Lo que llega se valida igual que en las rutas de la consola; lo que no vale se
//! ignora (lo dice el equipo, no una persona: no hay a quién contestar).

use crate::almacen::{ahora, Almacen, ClienteCtx, Equipo, Observacion, R};
use serde_json::{json, Value};

/// Lo que hay que cambiar aquí para que coincida con el equipo.
#[derive(Debug, Default, PartialEq)]
pub struct Cambios {
    pub nombre: Option<String>,
    pub etiquetas: Option<Vec<String>>,
    /// El texto (vacío: quitarla) y quién la cambió, en palabras.
    pub observacion: Option<(String, String)>,
}

impl Cambios {
    pub fn vacio(&self) -> bool {
        self.nombre.is_none() && self.etiquetas.is_none() && self.observacion.is_none()
    }
}

fn nombre_valido(n: &str) -> Option<String> {
    let n = n.trim();
    (!n.is_empty() && n.chars().count() <= 80 && !n.chars().any(char::is_control)).then(|| n.to_string())
}

fn observacion_valida(t: &str) -> Option<String> {
    let t = t.replace("\r\n", "\n").replace('\r', "\n");
    let t = t.trim();
    (t.chars().count() <= 2000 && !t.chars().any(|c| c.is_control() && c != '\n' && c != '\t')).then(|| t.to_string())
}

fn corto(t: &str, max: usize) -> String {
    t.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// Quién cambió un dato, en palabras: «Ana (desde la consola «Oficina»)».
fn quien(campo: &Value) -> String {
    let por = campo["por"].as_str().map(|p| corto(p, 60)).filter(|p| !p.is_empty());
    let consola = campo["consola"].as_str().map(|c| corto(c, 60)).filter(|c| !c.is_empty());
    match (por, consola) {
        (Some(p), Some(c)) => format!("{p} (desde la consola «{c}»)"),
        (Some(p), None) => format!("{p} (desde otra consola)"),
        (None, Some(c)) => format!("El equipo (desde la consola «{c}»)"),
        (None, None) => "El equipo".to_string(),
    }
}

/// Lo que dice el equipo (`resumen.datos_equipo`) frente a lo que hay aquí.
/// `observacion`: la observación que tiene aquí el equipo (`None`: ninguna).
pub fn cambios(e: &Equipo, observacion: Option<&str>, resumen: &Value) -> Cambios {
    let d = &resumen["datos_equipo"];
    let mut c = Cambios::default();
    if let Some(n) = d["nombre"]["valor"].as_str().and_then(nombre_valido).filter(|n| *n != e.nombre) {
        c.nombre = Some(n);
    }
    if let Some(xs) = d["etiquetas"]["valor"].as_array() {
        let textos: Option<Vec<String>> = xs.iter().map(|x| x.as_str().map(str::to_string)).collect();
        if let Some(xs) = textos.and_then(|t| crate::api::equipos::normalizar_etiquetas(&t).ok()).filter(|xs| *xs != e.etiquetas) {
            c.etiquetas = Some(xs);
        }
    }
    if let Some(t) = d["observacion"]["valor"].as_str().and_then(observacion_valida).filter(|t| t.as_str() != observacion.unwrap_or("")) {
        c.observacion = Some((t, quien(&d["observacion"])));
    }
    c
}

/// Copia aquí lo que dice el equipo (y lo audita, con el equipo como actor). Devuelve si cambió algo.
pub fn aplicar(db: &dyn Almacen, ctx: &ClienteCtx, e: &Equipo, resumen: &Value) -> R<bool> {
    if resumen["datos_equipo"].is_null() {
        return Ok(false);
    }
    let obs = db.observacion(ctx, "equipo", &e.id)?;
    let c = cambios(e, obs.as_ref().map(|o| o.texto.as_str()), resumen);
    if c.vacio() {
        return Ok(false);
    }
    let actor = format!("equipo:{}", e.id);
    let d = &resumen["datos_equipo"];
    if let Some(n) = &c.nombre {
        db.renombrar_equipo(ctx, &e.id, n)?;
        db.auditar(ctx, &actor, "renombrar_equipo", &e.id, &json!({ "nombre": n, "desde_equipo": true, "consola": d["nombre"]["consola"] }).to_string())?;
    }
    if let Some(xs) = &c.etiquetas {
        db.poner_etiquetas_equipo(ctx, &e.id, xs)?;
        db.auditar(
            ctx,
            &actor,
            "etiquetas_equipo",
            &e.id,
            &json!({ "etiquetas": xs, "desde_equipo": true, "consola": d["etiquetas"]["consola"] }).to_string(),
        )?;
    }
    if let Some((t, por)) = &c.observacion {
        let o = Observacion { tipo: "equipo".into(), objeto: e.id.clone(), texto: t.clone(), actualizada: ahora(), por_id: actor.clone(), por: por.clone() };
        if db.poner_observacion(ctx, &o, crate::api::notas::MAX_OBSERVACIONES)? {
            let datos = json!({ "tipo": "equipo", "caracteres": t.chars().count(), "borrada": t.is_empty(), "desde_equipo": true });
            db.auditar(ctx, &actor, "poner_observacion", &format!("equipo:{}", e.id), &datos.to_string())?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equipo() -> Equipo {
        Equipo {
            id: "e1".into(),
            nombre: "PC-RECEPCION".into(),
            so: "windows".into(),
            version_agente: "0.7.23".into(),
            box_pub: String::new(),
            sign_pub: String::new(),
            sal_equipo: String::new(),
            etiqueta: None,
            rol: "agente".into(),
            modo: "gestionado".into(),
            confirmado: true,
            ultimo_contacto: None,
            estado_servicio: None,
            siguiente_seq: 1,
            atencion_hasta: None,
            resumen: None,
            espera_min_horas: None,
            etiquetas: vec!["Oficina".into()],
        }
    }

    #[test]
    fn sin_datos_del_equipo_no_cambia_nada() {
        // Un agente anterior, o uno nuevo al que aún nadie le puso el nombre con la orden:
        // cada consola sigue con el suyo (no se pisa lo que ya había aquí).
        assert!(cambios(&equipo(), None, &json!({ "copias": [] })).vacio());
        assert!(cambios(&equipo(), Some("nota de aquí"), &json!({ "datos_equipo": null })).vacio());
        assert!(cambios(&equipo(), None, &json!({ "datos_equipo": {} })).vacio());
    }

    #[test]
    fn el_valor_del_equipo_manda() {
        let r = json!({ "datos_equipo": {
            "nombre": { "valor": " Recepción 2 ", "cuando": "2026-10-06T10:00:00+02:00", "consola": "Oficina", "esta": false, "por": "Ana" },
            "etiquetas": { "valor": ["Contabilidad", "contabilidad", " Sede  norte "], "consola": "Oficina" },
            "observacion": { "valor": "Disco nuevo\r\nel 3/10", "consola": "Oficina", "por": "Ana" },
        }});
        let c = cambios(&equipo(), Some("la de aquí"), &r);
        assert_eq!(c.nombre.as_deref(), Some("Recepción 2"));
        assert_eq!(c.etiquetas, Some(vec!["Contabilidad".to_string(), "Sede norte".to_string()]));
        assert_eq!(c.observacion, Some(("Disco nuevo\nel 3/10".to_string(), "Ana (desde la consola «Oficina»)".to_string())));
        // Lo que ya coincide no se vuelve a escribir.
        let mut e = equipo();
        e.nombre = "Recepción 2".into();
        e.etiquetas = vec!["Contabilidad".into(), "Sede norte".into()];
        assert!(cambios(&e, Some("Disco nuevo\nel 3/10"), &r).vacio());
        // Una observación vacía quita la de aquí (si había).
        let quitar = json!({ "datos_equipo": { "observacion": { "valor": "" } } });
        assert_eq!(cambios(&e, Some("vieja"), &quitar).observacion.map(|x| x.0), Some(String::new()));
        assert!(cambios(&e, None, &quitar).vacio());
    }

    #[test]
    fn lo_que_no_vale_se_ignora() {
        let r = json!({ "datos_equipo": {
            "nombre": { "valor": "" },
            "etiquetas": { "valor": ["a,b"] },
            "observacion": { "valor": "x\u{7}" },
        }});
        assert!(cambios(&equipo(), None, &r).vacio());
        let r = json!({ "datos_equipo": { "nombre": { "valor": "a".repeat(81) }, "etiquetas": { "valor": [1, 2] }, "observacion": { "valor": "ñ".repeat(2001) } } });
        assert!(cambios(&equipo(), None, &r).vacio());
        let r = json!({ "datos_equipo": { "etiquetas": { "valor": (0..11).map(|i| format!("e{i}")).collect::<Vec<_>>() } } });
        assert!(cambios(&equipo(), None, &r).vacio());
    }

    #[test]
    fn quien_sin_direcciones() {
        assert_eq!(quien(&json!({ "consola": "En línea" })), "El equipo (desde la consola «En línea»)");
        assert_eq!(quien(&json!({ "por": "Ana" })), "Ana (desde otra consola)");
        assert_eq!(quien(&json!({})), "El equipo");
        assert_eq!(quien(&json!({ "por": "x".repeat(200) })).chars().count(), 60 + " (desde otra consola)".chars().count());
    }
}
