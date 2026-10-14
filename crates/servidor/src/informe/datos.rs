//! Los datos del informe ejecutivo de un cliente en un periodo (plan 0.8.0, G1):
//! lo que cuenta el historial que sube cada equipo (copias, verificaciones,
//! pruebas de restauración y restauraciones), el estado de hoy de cada equipo,
//! el espacio, los avisos abiertos y la regla 3-2-1-1-0 (la última que calculó
//! la consola, ver [`super::FotoRegla`]).
//!
//! Solo metadatos: nombres de equipos y copias, cifras, fechas y mensajes ya sin
//! rutas ([`contenido::texto_publico`]). Nunca contraseñas, claves ni nombres de
//! archivos. Las listas tienen tope (el informe va entero en la cola de envíos).

use crate::almacen::{Almacen, ClienteCtx, Equipo, FiltroHistorial, Ts, R};
use crate::notificaciones::{contenido, resumen, zona, Severidad};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// Cada cuánto se hace el informe.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Periodo {
    #[default]
    Semanal,
    Mensual,
}

impl Periodo {
    pub fn de(s: &str) -> Option<Self> {
        match s {
            "semanal" => Some(Periodo::Semanal),
            "mensual" => Some(Periodo::Mensual),
            _ => None,
        }
    }
    pub fn clave(self) -> &'static str {
        match self {
            Periodo::Semanal => "semanal",
            Periodo::Mensual => "mensual",
        }
    }
    /// «semanal» / «mensual» dentro de una frase.
    pub fn adjetivo(self) -> &'static str {
        self.clave()
    }
}

/// Cómo acabaron las vueltas de algo (copias): correctas, sin cambios, con avisos y fallidas.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cuenta {
    pub ok: u32,
    pub sin_cambios: u32,
    pub aviso: u32,
    pub fallo: u32,
}

impl Cuenta {
    pub fn total(&self) -> u32 {
        self.ok + self.sin_cambios + self.aviso + self.fallo
    }
    /// Las correctas, como en Informes de la consola: bien o sin cambios (las de avisos, aparte).
    pub fn correctas(&self) -> u32 {
        self.ok + self.sin_cambios
    }
    /// El porcentaje de correctas (sin copias, ninguno).
    pub fn porcentaje(&self) -> Option<f64> {
        (self.total() > 0).then(|| self.correctas() as f64 * 100.0 / self.total() as f64)
    }
    fn sumar(&mut self, resultado: &str) {
        match resultado {
            "ok" => self.ok += 1,
            "sin_cambios" => self.sin_cambios += 1,
            "aviso" => self.aviso += 1,
            "fallo" => self.fallo += 1,
            _ => {}
        }
    }
}

/// Un día del periodo (desde su medianoche, hora del servidor).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Dia {
    pub inicio: Ts,
    pub copias: Cuenta,
    /// Datos nuevos guardados ese día.
    pub bytes: u64,
}

/// Una fila de la tabla de equipos.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FilaEquipo {
    pub nombre: String,
    /// El de hoy: `ok`, `aviso`, `fallo`, `sin_contacto` o `sin_datos` (como el resumen por correo).
    pub estado: String,
    pub copias: Cuenta,
    pub ultima_ok: Option<Ts>,
    pub bytes: Option<u64>,
}

/// Una copia que falló en el periodo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Fallo {
    pub equipo: String,
    pub copia: String,
    pub veces: u32,
    pub ultima: Ts,
    /// El último mensaje, sin rutas.
    pub mensaje: String,
}

/// Una verificación, prueba o restauración que falló.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FalloPrueba {
    pub equipo: String,
    pub repo: String,
    pub hora: Ts,
    pub mensaje: String,
}

/// Verificaciones o pruebas de restauración del periodo.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Pruebas {
    pub ok: u32,
    pub fallo: u32,
    pub fallidas: Vec<FalloPrueba>,
}

/// Restauraciones del periodo (la entrada `restauracion` del historial de cada equipo).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Restauraciones {
    pub ok: u32,
    pub fallo: u32,
    pub archivos: u64,
    pub bytes: u64,
    /// Lo que tardaron de media las correctas (el RTO real), si se sabe.
    pub duracion_media_s: Option<u64>,
}

/// El espacio: lo que ocupan hoy las copias y los datos nuevos del periodo y del anterior.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Espacio {
    pub protegido: u64,
    pub nuevos: u64,
    /// Los del periodo anterior, del mismo largo (para la tendencia).
    pub nuevos_anterior: u64,
}

/// Cómo está una parte de la regla en una copia (lo que calculó la consola).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EstadoParte {
    Cumple,
    /// La configuración cumple, pero algo no está al día.
    Atrasado,
    Falta,
}

/// Las cinco partes de la regla 3-2-1-1-0, en orden.
pub const PARTES: [&str; 5] = ["3 copias", "2 soportes", "1 fuera del sitio", "1 inmutable o aislado", "0 errores"];

/// Una copia que no cumple la regla, con lo que le falta.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CopiaRegla {
    pub equipo: String,
    pub copia: String,
    pub falta: String,
}

/// La regla 3-2-1-1-0 del cliente, según la última vez que la calculó la consola.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Regla {
    pub calculada: Ts,
    pub total: u32,
    pub cumplen: u32,
    /// Cuántas copias no cumplen cada parte (en el orden de [`PARTES`]).
    pub faltan: [u32; 5],
    pub sin_cumplir: Vec<CopiaRegla>,
}

/// Lo que le falta a una copia, en una línea (como `lineaFalta` de la consola):
/// «Falta: 1 fuera del sitio · 1 inmutable o aislado. No está al día: 0 errores.»
pub fn linea_falta(partes: &[EstadoParte; 5]) -> String {
    let de = |e: EstadoParte| partes.iter().zip(PARTES).filter(|(p, _)| **p == e).map(|(_, n)| n).collect::<Vec<_>>().join(" · ");
    let (falta, atrasado) = (de(EstadoParte::Falta), de(EstadoParte::Atrasado));
    let mut out = Vec::new();
    if !falta.is_empty() {
        out.push(format!("Falta: {falta}"));
    }
    if !atrasado.is_empty() {
        out.push(format!("No está al día: {atrasado}"));
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("{}.", out.join(". "))
    }
}

/// Un aviso importante que sigue abierto.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AvisoInforme {
    pub titulo: String,
    pub severidad: Severidad,
    pub veces: i64,
    pub desde: Ts,
}

/// El informe entero de un cliente en un periodo `[desde, hasta)`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Informe {
    pub cliente_id: String,
    pub cliente: String,
    pub periodo: Periodo,
    pub desde: Ts,
    pub hasta: Ts,
    pub generado: Ts,
    /// La zona del servidor al prepararlo (segundos al este de UTC).
    pub zona_s: i32,
    pub copias: Cuenta,
    pub dias: Vec<Dia>,
    pub equipos: Vec<FilaEquipo>,
    /// Los que no caben en la tabla.
    #[serde(default)]
    pub equipos_mas: u32,
    pub fallos: Vec<Fallo>,
    #[serde(default)]
    pub fallos_mas: u32,
    pub verificaciones: Pruebas,
    pub pruebas: Pruebas,
    pub restauraciones: Restauraciones,
    /// Plan 0.8.0, E3 (14k): las pruebas funcionales del periodo, los objetivos y el plazo.
    #[serde(default)]
    pub funcionales: crate::pruebas_funcionales::ParaInforme,
    pub espacio: Espacio,
    pub regla: Option<Regla>,
    pub avisos: Vec<AvisoInforme>,
    /// Quién lo prepara (el nombre del prestador, si lo puso el propietario del servidor).
    #[serde(default)]
    pub prestador: Option<String>,
}

/// Topes de las listas.
pub const MAX_EQUIPOS: usize = 300;
pub const MAX_FALLOS: usize = 40;
pub const MAX_PRUEBAS_FALLIDAS: usize = 20;
pub const MAX_AVISOS: usize = 15;
pub const MAX_SIN_CUMPLIR: usize = 40;

/// Los nombres de las copias y de los repositorios de un equipo (de su resumen).
fn nombres(e: &Equipo, campo: &str) -> HashMap<String, String> {
    e.resumen
        .as_ref()
        .and_then(|r| r[campo].as_array())
        .into_iter()
        .flatten()
        .filter_map(|x| Some((x["id"].as_str()?.to_string(), x["nombre"].as_str()?.to_string())))
        .collect()
}

/// Prepara el informe (`None` si el cliente no existe). `ahora` limita el último día (un
/// periodo en curso llega hasta hoy).
pub fn preparar(db: &dyn Almacen, cliente: &str, periodo: Periodo, desde: Ts, hasta: Ts, ahora: Ts) -> R<Option<Informe>> {
    let Some(c) = db.cliente(cliente)? else { return Ok(None) };
    let ctx = ClienteCtx::autorizado(cliente);
    let equipos: Vec<Equipo> = db.equipos(&ctx)?.into_iter().filter(|e| e.confirmado && e.modo == "gestionado").collect();
    let z = zona(hasta.min(ahora));
    let largo = (hasta - desde).max(1);
    let mut inf = Informe {
        cliente_id: c.id.clone(),
        cliente: c.nombre.clone(),
        periodo,
        desde,
        hasta,
        generado: ahora,
        zona_s: z.local_minus_utc(),
        copias: Cuenta::default(),
        dias: Vec::new(),
        equipos: Vec::new(),
        equipos_mas: 0,
        fallos: Vec::new(),
        fallos_mas: 0,
        verificaciones: Pruebas::default(),
        pruebas: Pruebas::default(),
        restauraciones: Restauraciones::default(),
        funcionales: crate::pruebas_funcionales::ParaInforme::default(),
        espacio: Espacio::default(),
        regla: None,
        avisos: Vec::new(),
        prestador: super::prestador(db)?.nombre,
    };

    // Los días del periodo (hasta hoy), desde su medianoche.
    let fin = hasta.min(ahora.max(desde + 1));
    let mut dia = desde;
    while dia < fin && inf.dias.len() < 62 {
        inf.dias.push(Dia { inicio: dia, ..Default::default() });
        dia = super::manana(dia);
    }
    let indice_dia = |t: Ts, dias: &[Dia]| dias.iter().rposition(|d| d.inicio <= t);

    // El estado de hoy de cada equipo (como el resumen por correo).
    let mut informes = HashMap::new();
    for e in &equipos {
        if let Some((_, v)) = db.informes(&ctx, &e.id, 1)?.into_iter().next() {
            informes.insert(e.id.clone(), v);
        }
    }
    let incidentes = db.notif_incidentes_abiertos(Some(cliente))?;
    let hoy = resumen::de_cliente(&c.id, &c.nombre, &equipos, &informes, &incidentes, ahora - 86_400, ahora);
    let hoy_de: HashMap<&str, &resumen::ResumenEquipo> = hoy.equipos.iter().map(|e| (e.id.as_str(), e)).collect();

    // (equipo, copia) → fallo.
    let mut fallos: BTreeMap<(String, String), Fallo> = BTreeMap::new();
    let mut duraciones: Vec<u64> = Vec::new();
    let filtro = FiltroHistorial {
        desde: Some(desde - largo - 1),
        hasta: Some(hasta - 1),
        tipos: vec!["copia".into(), "verificacion".into(), "prueba_restauracion".into(), "restauracion".into()],
        antes: None,
    };
    for e in &equipos {
        let copias = nombres(e, "copias");
        let repos = nombres(e, "repositorios");
        let mut fila = FilaEquipo {
            nombre: e.nombre.clone(),
            estado: hoy_de.get(e.id.as_str()).map_or("sin_datos".into(), |q| q.estado.clone()),
            copias: Cuenta::default(),
            ultima_ok: hoy_de.get(e.id.as_str()).and_then(|q| q.ultima_ok),
            bytes: hoy_de.get(e.id.as_str()).and_then(|q| q.bytes),
        };
        for h in db.historial(&ctx, &e.id, &filtro, crate::almacen::MAX_HISTORIAL_EQUIPO)? {
            let Ok(v) = serde_json::from_str::<Value>(&h.datos) else { continue };
            let resultado = v["resultado"].as_str().unwrap_or("");
            let en_periodo = h.hora >= desde && h.hora < hasta;
            let mensaje = || contenido::texto_publico(v["mensaje"].as_str().unwrap_or(""));
            let repo = || {
                let id = v["repo"].as_str().unwrap_or("");
                repos.get(id).cloned().unwrap_or_else(|| if id.is_empty() { "—".into() } else { "Repositorio quitado".into() })
            };
            match h.tipo.as_str() {
                "copia" => {
                    let bytes = if resultado == "ok" || resultado == "aviso" { v["anadido"].as_u64().unwrap_or(0) } else { 0 };
                    // La última correcta, también del historial (el último informe del equipo puede no traerla).
                    if matches!(resultado, "ok" | "sin_cambios" | "aviso") {
                        fila.ultima_ok = fila.ultima_ok.max(Some(h.hora));
                    }
                    if !en_periodo {
                        if h.hora < desde {
                            inf.espacio.nuevos_anterior += bytes;
                        }
                        continue;
                    }
                    inf.copias.sumar(resultado);
                    fila.copias.sumar(resultado);
                    inf.espacio.nuevos += bytes;
                    if let Some(i) = indice_dia(h.hora, &inf.dias) {
                        inf.dias[i].copias.sumar(resultado);
                        inf.dias[i].bytes += bytes;
                    }
                    if resultado == "fallo" {
                        let id = v["copia"].as_str().unwrap_or("").to_string();
                        let copia = copias.get(&id).cloned().unwrap_or_else(|| "Copia quitada".into());
                        let f = fallos.entry((e.nombre.clone(), id)).or_insert_with(|| Fallo {
                            equipo: e.nombre.clone(),
                            copia,
                            veces: 0,
                            ultima: h.hora,
                            mensaje: String::new(),
                        });
                        f.veces += 1;
                        if h.hora >= f.ultima || f.mensaje.is_empty() {
                            f.ultima = f.ultima.max(h.hora);
                            let m = mensaje();
                            if !m.is_empty() {
                                f.mensaje = m;
                            }
                        }
                    }
                }
                "verificacion" | "prueba_restauracion" if en_periodo => {
                    let p = if h.tipo == "verificacion" { &mut inf.verificaciones } else { &mut inf.pruebas };
                    if resultado == "fallo" {
                        p.fallo += 1;
                        if p.fallidas.len() < MAX_PRUEBAS_FALLIDAS {
                            p.fallidas.push(FalloPrueba { equipo: e.nombre.clone(), repo: repo(), hora: h.hora, mensaje: mensaje() });
                        }
                    } else if !resultado.is_empty() {
                        p.ok += 1;
                    }
                }
                "restauracion" if en_periodo => {
                    let r = &mut inf.restauraciones;
                    if resultado == "fallo" {
                        r.fallo += 1;
                    } else if !resultado.is_empty() {
                        r.ok += 1;
                        r.archivos += v["archivos"].as_u64().unwrap_or(0);
                        r.bytes += v["bytes"].as_u64().unwrap_or(0);
                        if let Some(d) = v["duracion_s"].as_u64() {
                            duraciones.push(d);
                        }
                    }
                }
                _ => {}
            }
        }
        inf.espacio.protegido += fila.bytes.unwrap_or(0);
        inf.equipos.push(fila);
    }
    inf.restauraciones.duracion_media_s = (!duraciones.is_empty()).then(|| duraciones.iter().sum::<u64>() / duraciones.len() as u64);
    for p in [&mut inf.verificaciones, &mut inf.pruebas] {
        p.fallidas.sort_by_key(|f| std::cmp::Reverse(f.hora));
    }

    // Primero lo que peor está (como el resumen por correo).
    let peso = |s: &str| match s {
        "fallo" => 0,
        "sin_contacto" => 1,
        "aviso" => 2,
        "sin_datos" => 3,
        _ => 4,
    };
    inf.equipos.sort_by(|a, b| peso(&a.estado).cmp(&peso(&b.estado)).then_with(|| a.nombre.to_lowercase().cmp(&b.nombre.to_lowercase())));
    if inf.equipos.len() > MAX_EQUIPOS {
        inf.equipos_mas = (inf.equipos.len() - MAX_EQUIPOS) as u32;
        inf.equipos.truncate(MAX_EQUIPOS);
    }
    let mut fallos: Vec<Fallo> = fallos.into_values().collect();
    fallos.sort_by(|a, b| b.veces.cmp(&a.veces).then(b.ultima.cmp(&a.ultima)));
    if fallos.len() > MAX_FALLOS {
        inf.fallos_mas = (fallos.len() - MAX_FALLOS) as u32;
        fallos.truncate(MAX_FALLOS);
    }
    inf.fallos = fallos;

    // Los avisos importantes que siguen abiertos.
    let mut avisos: Vec<&crate::notificaciones::Incidente> = incidentes.iter().filter(|i| i.abierto && i.severidad >= Severidad::Importante).collect();
    avisos.sort_by(|a, b| b.severidad.cmp(&a.severidad).then(b.ultimo.cmp(&a.ultimo)));
    inf.avisos = avisos
        .into_iter()
        .take(MAX_AVISOS)
        .map(|i| AvisoInforme { titulo: contenido::texto_publico(&i.titulo), severidad: i.severidad, veces: i.veces, desde: i.primero })
        .collect();

    // E3 (14k): las pruebas funcionales del periodo.
    inf.funcionales = crate::pruebas_funcionales::para_informe(db, &ctx, &equipos, desde, hasta, ahora)?;

    // La regla 3-2-1-1-0: la última que calculó la consola (solo de los equipos que siguen).
    if let Some(foto) = super::foto_regla(db, cliente)? {
        let por_id: HashMap<&str, &Equipo> = equipos.iter().map(|e| (e.id.as_str(), e)).collect();
        let mut regla = Regla { calculada: foto.calculada, total: 0, cumplen: 0, faltan: [0; 5], sin_cumplir: Vec::new() };
        let mut copias_de: HashMap<&str, HashMap<String, String>> = HashMap::new();
        for x in &foto.copias {
            let Some(e) = por_id.get(x.equipo.as_str()) else { continue };
            regla.total += 1;
            for (i, p) in x.partes.iter().enumerate() {
                if *p != EstadoParte::Cumple {
                    regla.faltan[i] += 1;
                }
            }
            if x.partes.iter().all(|p| *p == EstadoParte::Cumple) {
                regla.cumplen += 1;
            } else if regla.sin_cumplir.len() < MAX_SIN_CUMPLIR {
                let copias = copias_de.entry(e.id.as_str()).or_insert_with(|| nombres(e, "copias"));
                regla.sin_cumplir.push(CopiaRegla {
                    equipo: e.nombre.clone(),
                    copia: copias.get(&x.copia).cloned().unwrap_or_else(|| "Copia quitada".into()),
                    falta: linea_falta(&x.partes),
                });
            }
        }
        inf.regla = (regla.total > 0).then_some(regla);
    }
    Ok(Some(inf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuentas_y_lo_que_falta() {
        let mut c = Cuenta::default();
        for r in ["ok", "ok", "sin_cambios", "aviso", "fallo", "otra"] {
            c.sumar(r);
        }
        assert_eq!((c.total(), c.correctas()), (5, 3));
        assert_eq!(c.porcentaje(), Some(60.0));
        assert_eq!(Cuenta::default().porcentaje(), None);
        use EstadoParte::*;
        assert_eq!(linea_falta(&[Cumple; 5]), "");
        assert_eq!(linea_falta(&[Cumple, Cumple, Falta, Falta, Cumple]), "Falta: 1 fuera del sitio · 1 inmutable o aislado.");
        assert_eq!(linea_falta(&[Cumple, Cumple, Cumple, Cumple, Atrasado]), "No está al día: 0 errores.");
        assert_eq!(linea_falta(&[Falta, Cumple, Cumple, Cumple, Atrasado]), "Falta: 3 copias. No está al día: 0 errores.");
    }
}
