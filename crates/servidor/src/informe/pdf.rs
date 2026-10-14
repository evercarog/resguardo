//! El PDF del informe ejecutivo (plan 0.8.0, G1), escrito aquí mismo con
//! pdf-writer: sin navegador ni programas de fuera.
//!
//! Sobrio y legible, con los colores de la consola (docs/diseno.md §2: el color
//! solo para el estado, y siempre con su texto): portada con el logo, una
//! frase, cuatro cifras, dos gráficas (copias por día y datos nuevos por día),
//! la regla 3-2-1-1-0, verificaciones, pruebas y restauraciones, la tabla de
//! equipos, lo que falló y los avisos abiertos. A4, Helvetica (una de las 14
//! fuentes de todo lector de PDF: no se incrusta ninguna) con WinAnsiEncoding,
//! que tiene todas las letras del español; lo que no cabe en ella sale como «?».
//!
//! El logo (PNG ya comprobado al subirlo) se vuelve a leer aquí con límites; si
//! no se puede leer, el informe sale sin él.

use super::datos::{Cuenta, Informe, Periodo, PARTES};
use crate::notificaciones::Severidad;
use chrono::{Datelike, FixedOffset, Timelike};
use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

const ANCHO: f32 = 595.0;
const ALTO: f32 = 842.0;
const MARGEN: f32 = 48.0;
/// Lo más abajo que llega el contenido (debajo, el pie).
const SUELO: f32 = MARGEN + 12.0;
const UTIL: f32 = ANCHO - 2.0 * MARGEN;

// Colores de la consola, en claro (docs/diseno.md §2).
const TEXTO_1: u32 = 0x18181b;
const TEXTO_2: u32 = 0x52525b;
const TEXTO_3: u32 = 0x666670;
const BORDE: u32 = 0xe9e9ec;
const BORDE_FUERTE: u32 = 0xd5d5da;
const SUPERFICIE_2: u32 = 0xf4f4f5;
const ACENTO: u32 = 0x0f766e;
const OK: u32 = 0x126c35;
const WARN: u32 = 0xa04e09;
const BAD: u32 = 0xc42121;
/// Las barras de una gráfica sin estado (`--graf-marca`: `--text-3` al 75 %).
const MARCA: u32 = 0x8c8c94;

const MESES: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
const MESES_CORTOS: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];
const DIAS_CORTOS: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum F {
    Normal,
    Negrita,
}

impl F {
    fn nombre(self) -> Name<'static> {
        match self {
            F::Normal => Name(b"R"),
            F::Negrita => Name(b"B"),
        }
    }
}

// ---------- Texto: WinAnsiEncoding y anchos de Helvetica ----------

/// El texto en WinAnsiEncoding (lo que no está, «?»).
fn win(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            ' '..='~' => c as u8,
            '\u{a0}'..='\u{ff}' => c as u32 as u8,
            '€' => 0x80,
            '‚' => 0x82,
            '„' => 0x84,
            '…' => 0x85,
            '‰' => 0x89,
            '‹' => 0x8b,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '™' => 0x99,
            '›' => 0x9b,
            '\u{2009}' | '\u{202f}' | '\t' => b' ',
            _ => b'?',
        })
        .collect()
}

/// Anchos (milésimas de em) de Helvetica y Helvetica-Bold, de « » a «~».
const ANCHOS_R: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584,
    556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278,
    469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260,
    334, 584,
];
const ANCHOS_B: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584,
    611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333,
    584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280,
    389, 584,
];

/// El ancho de un byte de WinAnsi (las letras con tilde, como su letra sin ella).
fn ancho_byte(b: u8, f: F) -> u16 {
    let t = if f == F::Negrita { &ANCHOS_B } else { &ANCHOS_R };
    let base = |c: u8| t[(c - 32) as usize];
    match b {
        32..=126 => base(b),
        0xc0..=0xc5 => base(b'A'),
        0xc7 => base(b'C'),
        0xc8..=0xcb => base(b'E'),
        0xcc..=0xcf | 0xec..=0xef => 278,
        0xd1 => base(b'N'),
        0xd2..=0xd6 | 0xd8 => base(b'O'),
        0xd9..=0xdc => base(b'U'),
        0xdd => base(b'Y'),
        0xe0..=0xe5 => base(b'a'),
        0xe7 => base(b'c'),
        0xe8..=0xeb => base(b'e'),
        0xf1 => base(b'n'),
        0xf2..=0xf6 | 0xf8 => base(b'o'),
        0xf9..=0xfc => base(b'u'),
        0xfd | 0xff => base(b'y'),
        0xa0 | 0xb7 => 278,
        0xa1 => 333,
        0xbf => 611,
        0xb0 => 400,
        0xd7 => 584,
        0x85 | 0x97 | 0x89 | 0x99 => 1000,
        0x91 | 0x92 => 278,
        0x93 | 0x94 => 500,
        0x95 => 350,
        _ => 556,
    }
}

fn ancho(s: &str, f: F, tam: f32) -> f32 {
    win(s).iter().map(|b| ancho_byte(*b, f) as f32).sum::<f32>() * tam / 1000.0
}

/// Corta el texto para que quepa en `max` (con «…»).
fn recortar(s: &str, f: F, tam: f32, max: f32) -> String {
    if ancho(s, f, tam) <= max {
        return s.to_string();
    }
    let mut v: Vec<char> = s.chars().collect();
    while !v.is_empty() {
        v.pop();
        let t: String = v.iter().collect::<String>().trim_end().to_string() + "…";
        if ancho(&t, f, tam) <= max {
            return t;
        }
    }
    "…".into()
}

/// Parte el texto en líneas de `max` de ancho (por palabras; una palabra que no cabe, cortada).
fn lineas(s: &str, f: F, tam: f32, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    for parrafo in s.split('\n') {
        let mut linea = String::new();
        for palabra in parrafo.split(' ').filter(|p| !p.is_empty()) {
            let prueba = if linea.is_empty() { palabra.to_string() } else { format!("{linea} {palabra}") };
            if ancho(&prueba, f, tam) <= max {
                linea = prueba;
            } else {
                if !linea.is_empty() {
                    out.push(std::mem::take(&mut linea));
                }
                linea = recortar(palabra, f, tam, max);
            }
        }
        out.push(linea);
    }
    out
}

// ---------- Fechas y cifras ----------

fn en_zona(ts: i64, z: FixedOffset) -> chrono::DateTime<FixedOffset> {
    chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default().with_timezone(&z)
}

/// «5 de octubre de 2026».
fn fecha_larga(ts: i64, z: FixedOffset) -> String {
    let d = en_zona(ts, z);
    format!("{} de {} de {}", d.day(), MESES[d.month0() as usize], d.year())
}

/// «5 oct, 10:32».
fn fecha_corta(ts: i64, z: FixedOffset) -> String {
    let d = en_zona(ts, z);
    format!("{} {}, {:02}:{:02}", d.day(), MESES_CORTOS[d.month0() as usize], d.hour(), d.minute())
}

/// El periodo en palabras: «del 5 al 11 de octubre de 2026», «octubre de 2026».
pub fn texto_periodo(i: &Informe) -> String {
    let z = zona(i);
    let (a, b) = (en_zona(i.desde, z), en_zona(i.hasta - 1, z));
    match i.periodo {
        Periodo::Mensual if a.day() == 1 && a.month() == b.month() => format!("{} de {}", MESES[a.month0() as usize], a.year()),
        _ if a.year() != b.year() => format!("del {} al {}", fecha_larga(i.desde, z), fecha_larga(i.hasta - 1, z)),
        _ if a.month() != b.month() => {
            format!("del {} de {} al {} de {} de {}", a.day(), MESES[a.month0() as usize], b.day(), MESES[b.month0() as usize], b.year())
        }
        _ => format!("del {} al {} de {} de {}", a.day(), b.day(), MESES[b.month0() as usize], b.year()),
    }
}

fn zona(i: &Informe) -> FixedOffset {
    FixedOffset::east_opt(i.zona_s).unwrap_or_else(|| FixedOffset::east_opt(0).expect("cero"))
}

/// «1.234».
fn numero(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// «97 %», «99,4 %» (sin redondear a 100 lo que no lo es).
fn porcentaje(p: f64) -> String {
    let t = if p >= 100.0 {
        "100".to_string()
    } else if p >= 99.95 {
        "99,9".to_string()
    } else if (p - p.round()).abs() < 0.05 {
        format!("{:.0}", p)
    } else {
        format!("{:.1}", p).replace('.', ",")
    };
    format!("{t}\u{a0}%")
}

fn plural(n: u64, uno: &str, varios: &str) -> String {
    if n == 1 {
        format!("1 {uno}")
    } else {
        format!("{} {varios}", numero(n))
    }
}

/// «35 s», «4 min», «1 h 12 min».
fn duracion(s: u64) -> String {
    match s {
        0..=59 => format!("{s} s"),
        60..=3599 => format!("{} min", (s + 30) / 60),
        _ => format!("{} h {} min", s / 3600, (s % 3600) / 60),
    }
}

fn tamano(n: u64) -> String {
    crate::notificaciones::contenido::tamano(n)
}

/// Lo que se ve del estado de hoy de un equipo: (texto, color).
fn estado_equipo(e: &str) -> (&'static str, u32) {
    match e {
        "fallo" => ("Con fallos", BAD),
        "sin_contacto" => ("Sin contacto", WARN),
        "aviso" => ("Revisar", WARN),
        "sin_datos" => ("Sin datos", TEXTO_3),
        _ => ("Bien", OK),
    }
}

// ---------- El lienzo ----------

fn rgb(c: u32) -> (f32, f32, f32) {
    (((c >> 16) & 0xff) as f32 / 255.0, ((c >> 8) & 0xff) as f32 / 255.0, (c & 0xff) as f32 / 255.0)
}

/// Una celda de una tabla.
#[derive(Clone, Default)]
struct Celda {
    texto: String,
    color: Option<u32>,
    /// Un punto de color delante (el estado: siempre con su texto).
    punto: Option<u32>,
    negrita: bool,
}

fn celda(t: impl Into<String>) -> Celda {
    Celda { texto: t.into(), ..Default::default() }
}

struct Columna {
    titulo: &'static str,
    ancho: f32,
    derecha: bool,
    /// Puede ocupar dos líneas.
    dos_lineas: bool,
}

const fn col(titulo: &'static str, ancho: f32) -> Columna {
    Columna { titulo, ancho, derecha: false, dos_lineas: false }
}
const fn col_d(titulo: &'static str, ancho: f32) -> Columna {
    Columna { titulo, ancho, derecha: true, dos_lineas: false }
}
const fn col_2(titulo: &'static str, ancho: f32) -> Columna {
    Columna { titulo, ancho, derecha: false, dos_lineas: true }
}

struct Lienzo {
    paginas: Vec<Content>,
    /// La página en la que se dibuja.
    actual: usize,
    y: f32,
    /// Todo lo escrito, en orden (para las pruebas: que no lleva lo que no debe).
    escrito: Vec<String>,
}

impl Lienzo {
    fn nuevo() -> Self {
        let mut l = Self { paginas: Vec::new(), actual: 0, y: 0.0, escrito: Vec::new() };
        l.nueva_pagina();
        l
    }

    fn nueva_pagina(&mut self) {
        self.paginas.push(Content::new());
        self.actual = self.paginas.len() - 1;
        self.y = ALTO - MARGEN;
    }

    fn c(&mut self) -> &mut Content {
        if self.actual >= self.paginas.len() {
            self.nueva_pagina();
        }
        &mut self.paginas[self.actual]
    }

    /// Hace sitio para `h` (si no cabe, página nueva). Devuelve si cambió de página.
    fn sitio(&mut self, h: f32) -> bool {
        if self.y - h < SUELO {
            self.nueva_pagina();
            true
        } else {
            false
        }
    }

    fn texto(&mut self, x: f32, y: f32, t: &str, f: F, tam: f32, color: u32) {
        if t.is_empty() {
            return;
        }
        self.escrito.push(t.to_string());
        let (r, g, b) = rgb(color);
        let bytes = win(t);
        let c = self.c();
        c.begin_text();
        c.set_font(f.nombre(), tam);
        c.set_fill_rgb(r, g, b);
        c.next_line(x, y);
        c.show(Str(&bytes));
        c.end_text();
    }

    fn texto_derecha(&mut self, x_der: f32, y: f32, t: &str, f: F, tam: f32, color: u32) {
        let w = ancho(t, f, tam);
        self.texto(x_der - w, y, t, f, tam, color);
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        let (r, g, b) = rgb(color);
        let c = self.c();
        c.set_fill_rgb(r, g, b);
        c.rect(x, y, w, h);
        c.fill_nonzero();
    }

    /// Un rectángulo con las esquinas redondeadas (radio `r`).
    fn rect_redondo(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
        let (cr, cg, cb) = rgb(color);
        let k = 0.552_284_8 * r;
        let c = self.c();
        c.set_fill_rgb(cr, cg, cb);
        c.move_to(x + r, y);
        c.line_to(x + w - r, y);
        c.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
        c.line_to(x + w, y + h - r);
        c.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
        c.line_to(x + r, y + h);
        c.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
        c.line_to(x, y + r);
        c.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
        c.close_path();
        c.fill_nonzero();
    }

    fn circulo(&mut self, x: f32, y: f32, r: f32, color: u32) {
        self.rect_redondo(x - r, y - r, 2.0 * r, 2.0 * r, r, color);
    }

    fn linea(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: u32, grosor: f32, discontinua: bool) {
        let (r, g, b) = rgb(color);
        let c = self.c();
        c.save_state();
        c.set_stroke_rgb(r, g, b);
        c.set_line_width(grosor);
        if discontinua {
            c.set_dash_pattern([2.0, 3.0], 0.0);
        }
        c.move_to(x1, y1);
        c.line_to(x2, y2);
        c.stroke();
        c.restore_state();
    }

    /// Un párrafo desde `self.y`, que pasa de página si hace falta.
    fn parrafo(&mut self, t: &str, f: F, tam: f32, color: u32) {
        let inter = tam * 1.45;
        for l in lineas(t, f, tam, UTIL) {
            self.sitio(inter);
            self.texto(MARGEN, self.y - tam, &l, f, tam, color);
            self.y -= inter;
        }
    }

    /// El título de una sección (con sitio para lo que va debajo: `debajo`).
    fn seccion(&mut self, t: &str, debajo: f32) {
        self.y -= 14.0;
        self.sitio(24.0 + debajo);
        self.texto(MARGEN, self.y - 13.0, t, F::Negrita, 13.0, TEXTO_1);
        self.y -= 22.0;
    }

    fn cabecera_tabla(&mut self, cols: &[Columna]) {
        let mut x = MARGEN;
        for c in cols {
            if c.derecha {
                self.texto_derecha(x + c.ancho - 10.0, self.y - 9.0, c.titulo, F::Negrita, 7.5, TEXTO_3);
            } else {
                self.texto(x, self.y - 9.0, c.titulo, F::Negrita, 7.5, TEXTO_3);
            }
            x += c.ancho;
        }
        self.y -= 14.0;
        self.linea(MARGEN, self.y, MARGEN + UTIL, self.y, BORDE_FUERTE, 0.75, false);
    }

    /// Una tabla: cabecera (repetida en cada página) y filas con una línea fina entre ellas.
    fn tabla(&mut self, cols: &[Columna], filas: &[Vec<Celda>]) {
        const TAM: f32 = 9.0;
        const INTER: f32 = 11.5;
        self.sitio(14.0 + 22.0);
        self.cabecera_tabla(cols);
        for fila in filas {
            let partidas: Vec<Vec<String>> = cols
                .iter()
                .zip(fila)
                .map(|(c, x)| {
                    let f = if x.negrita { F::Negrita } else { F::Normal };
                    let max = c.ancho - if c.derecha { 12.0 } else { 6.0 } - if x.punto.is_some() { 9.0 } else { 0.0 };
                    let mut ls = lineas(&x.texto, f, TAM, max);
                    if !c.dos_lineas || ls.len() > 2 {
                        let resto = ls.split_off(if c.dos_lineas { 1 } else { 0 }).join(" ");
                        ls.push(recortar(&resto, f, TAM, max));
                    }
                    ls
                })
                .collect();
            let n = partidas.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let alto = 7.0 + n as f32 * INTER;
            if self.sitio(alto) {
                self.cabecera_tabla(cols);
            }
            let mut x = MARGEN;
            for ((c, celda), ls) in cols.iter().zip(fila).zip(&partidas) {
                let f = if celda.negrita { F::Negrita } else { F::Normal };
                let color = celda.color.unwrap_or(TEXTO_1);
                let mut xt = x;
                if let Some(p) = celda.punto {
                    self.circulo(x + 3.0, self.y - 4.0 - TAM * 0.36, 2.6, p);
                    xt += 9.0;
                }
                for (k, l) in ls.iter().enumerate() {
                    let yl = self.y - 4.0 - TAM - k as f32 * INTER + 1.5;
                    if c.derecha {
                        self.texto_derecha(x + c.ancho - 10.0, yl, l, f, TAM, color);
                    } else {
                        self.texto(xt, yl, l, f, TAM, color);
                    }
                }
                x += c.ancho;
            }
            self.y -= alto;
            self.linea(MARGEN, self.y, MARGEN + UTIL, self.y, BORDE, 0.5, false);
        }
    }
}

// ---------- El logo ----------

struct Imagen {
    ancho: u32,
    alto: u32,
    rgb: Vec<u8>,
    alfa: Option<Vec<u8>>,
}

/// Lee el PNG (con límites: 2048 px de lado y 32 MB al descomprimir); `None` si no se puede.
fn imagen(png_bytes: &[u8]) -> Option<Imagen> {
    let mut d = png::Decoder::new_with_limits(std::io::Cursor::new(png_bytes), png::Limits { bytes: 32 * 1024 * 1024 });
    d.set_transformations(png::Transformations::normalize_to_color8());
    let mut r = d.read_info().ok()?;
    {
        let info = r.info();
        if info.width == 0 || info.height == 0 || info.width > crate::api::marca::MAX_LADO || info.height > crate::api::marca::MAX_LADO {
            return None;
        }
    }
    let mut buf = vec![0; r.output_buffer_size()?];
    let salida = r.next_frame(&mut buf).ok()?;
    let px = buf.get(..salida.buffer_size())?;
    let (rgb, alfa): (Vec<u8>, Option<Vec<u8>>) = match salida.color_type {
        png::ColorType::Grayscale => (px.iter().flat_map(|g| [*g, *g, *g]).collect(), None),
        png::ColorType::GrayscaleAlpha => (px.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0]]).collect(), Some(px.chunks_exact(2).map(|p| p[1]).collect())),
        png::ColorType::Rgb => (px.to_vec(), None),
        png::ColorType::Rgba => (px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(), Some(px.chunks_exact(4).map(|p| p[3]).collect())),
        png::ColorType::Indexed => return None,
    };
    if rgb.len() != salida.width as usize * salida.height as usize * 3 {
        return None;
    }
    Some(Imagen {
        ancho: salida.width,
        alto: salida.height,
        rgb: compress_to_vec_zlib(&rgb, 6),
        alfa: alfa.filter(|a| a.iter().any(|x| *x < 255)).map(|a| compress_to_vec_zlib(&a, 6)),
    })
}

// ---------- El informe ----------

/// La frase del principio: qué pasó en el periodo y cómo está hoy.
pub fn frase(i: &Informe) -> String {
    let en = match i.periodo {
        Periodo::Semanal => "En la semana",
        Periodo::Mensual => "En el mes",
    };
    let c = &i.copias;
    let mut t = if c.total() == 0 {
        format!("{en} no se hizo ninguna copia.")
    } else {
        let con: Vec<String> = i.equipos.iter().filter(|e| e.copias.total() > 0).map(|_| String::new()).collect();
        let mut s = format!(
            "{en}, {} {} {}: {} correctas",
            numero(con.len() as u64),
            if con.len() == 1 { "equipo hizo" } else { "equipos hicieron" },
            plural(c.total() as u64, "copia", "copias"),
            porcentaje(c.porcentaje().unwrap_or(0.0)),
        );
        match (c.aviso, c.fallo) {
            (0, 0) => {}
            (a, 0) => s.push_str(&format!(" y {} con avisos", numero(a as u64))),
            (0, f) => s.push_str(&format!(" y {}", plural(f as u64, "fallida", "fallidas"))),
            (a, f) => s.push_str(&format!(", {} con avisos y {}", numero(a as u64), plural(f as u64, "fallida", "fallidas"))),
        }
        s.push('.');
        s
    };
    if i.espacio.nuevos > 0 {
        t.push_str(&format!(" Se guardaron {} nuevos.", tamano(i.espacio.nuevos)));
    }
    let mal = i.equipos.iter().filter(|e| matches!(e.estado.as_str(), "fallo" | "sin_contacto" | "aviso")).count() as u64;
    if mal > 0 {
        t.push_str(&format!(" Hoy {} atención.", plural(mal, "equipo necesita", "equipos necesitan")));
    } else if !i.equipos.is_empty() {
        t.push_str(" Hoy todo está protegido.");
    }
    t
}

/// La tendencia de los datos nuevos frente al periodo anterior, si se puede decir.
fn tendencia(i: &Informe) -> Option<String> {
    let (a, b) = (i.espacio.nuevos as f64, i.espacio.nuevos_anterior as f64);
    if b <= 0.0 {
        return None;
    }
    let d = (a - b) * 100.0 / b;
    Some(if d.abs() < 5.0 {
        "parecido al periodo anterior".into()
    } else {
        format!("{}{:.0}\u{a0}% respecto al periodo anterior", if d > 0.0 { "+" } else { "−" }, d.abs())
    })
}

/// Un tope «redondo» para el eje (1, 2 o 5 por una potencia de 10).
fn tope(max: f64) -> f64 {
    if max <= 0.0 {
        return 1.0;
    }
    let p = 10f64.powf(max.log10().floor());
    // Con la mitad redonda (la rejilla va en 0, la mitad y el tope).
    for k in [1.0, 2.0, 4.0, 5.0, 10.0] {
        if k * p >= max {
            return k * p;
        }
    }
    10.0 * p
}

fn cabecera(l: &mut Lienzo, i: &Informe, logo: Option<(f32, f32)>) {
    let z = zona(i);
    let arriba = l.y;
    match logo {
        Some((w, h)) => {
            let c = l.c();
            c.save_state();
            c.transform([w, 0.0, 0.0, h, MARGEN, arriba - h]);
            c.x_object(Name(b"L"));
            c.restore_state();
        }
        None => {
            l.rect_redondo(MARGEN, arriba - 14.0, 12.0, 12.0, 3.0, ACENTO);
            let nombre = i.prestador.clone().unwrap_or_else(|| "Resguardo".into());
            l.texto(MARGEN + 18.0, arriba - 13.0, &recortar(&nombre, F::Negrita, 13.0, 260.0), F::Negrita, 13.0, TEXTO_1);
        }
    }
    let tipo = match i.periodo {
        Periodo::Semanal => "INFORME SEMANAL",
        Periodo::Mensual => "INFORME MENSUAL",
    };
    l.texto_derecha(MARGEN + UTIL, arriba - 10.0, tipo, F::Negrita, 8.0, TEXTO_3);
    l.texto_derecha(MARGEN + UTIL, arriba - 22.0, &format!("Preparado el {}", fecha_corta(i.generado, z)), F::Normal, 8.0, TEXTO_3);
    l.y = arriba - 36.0 - 26.0;
    let titulo = match i.periodo {
        Periodo::Semanal => "Informe semanal de copias",
        Periodo::Mensual => "Informe mensual de copias",
    };
    l.texto(MARGEN, l.y - 22.0, titulo, F::Negrita, 22.0, TEXTO_1);
    l.y -= 32.0;
    let sub = format!("{} · {}", i.cliente, texto_periodo(i));
    l.texto(MARGEN, l.y - 12.0, &recortar(&sub, F::Negrita, 12.0, UTIL), F::Negrita, 12.0, TEXTO_2);
    l.y -= 24.0;
    l.parrafo(&frase(i), F::Normal, 10.5, TEXTO_2);
}

/// Cuatro cifras: copias correctas, fallidas, equipos que necesitan atención y lo protegido.
fn cifras(l: &mut Lienzo, i: &Informe) {
    l.y -= 10.0;
    let alto = 62.0;
    l.sitio(alto);
    let ancho = (UTIL - 3.0 * 10.0) / 4.0;
    let mal = i.equipos.iter().filter(|e| matches!(e.estado.as_str(), "fallo" | "sin_contacto" | "aviso")).count() as u64;
    let total_equipos = i.equipos.len() as u64 + i.equipos_mas as u64;
    let c = &i.copias;
    let cifras: [(String, &str, String, u32); 4] = [
        (
            c.porcentaje().map(porcentaje).unwrap_or_else(|| "—".into()),
            "copias correctas",
            plural(c.total() as u64, "copia en total", "copias en total"),
            TEXTO_1,
        ),
        (
            numero(c.fallo as u64),
            if c.fallo == 1 { "copia fallida" } else { "copias fallidas" },
            if c.aviso > 0 { format!("{} con avisos", numero(c.aviso as u64)) } else { String::new() },
            if c.fallo > 0 { BAD } else { TEXTO_1 },
        ),
        (format!("{} de {}", numero(mal), numero(total_equipos)), "equipos que necesitan atención hoy", String::new(), if mal > 0 { WARN } else { TEXTO_1 }),
        (tamano(i.espacio.protegido), "protegido hoy", format!("{} nuevos", tamano(i.espacio.nuevos)), TEXTO_1),
    ];
    for (k, (valor, etiqueta, sub, color)) in cifras.iter().enumerate() {
        let x = MARGEN + k as f32 * (ancho + 10.0);
        l.rect_redondo(x, l.y - alto, ancho, alto, 6.0, SUPERFICIE_2);
        l.texto(x + 10.0, l.y - 24.0, &recortar(valor, F::Negrita, 17.0, ancho - 20.0), F::Negrita, 17.0, *color);
        for (n, ln) in lineas(etiqueta, F::Normal, 8.0, ancho - 20.0).iter().take(2).enumerate() {
            l.texto(x + 10.0, l.y - 36.0 - n as f32 * 9.5, ln, F::Normal, 8.0, TEXTO_3);
        }
        if !sub.is_empty() {
            l.texto(x + 10.0, l.y - alto + 8.0, &recortar(sub, F::Normal, 7.5, ancho - 20.0), F::Normal, 7.5, TEXTO_3);
        }
    }
    l.y -= alto;
}

/// Una leyenda a la derecha del título de una sección: cuadros de color con su texto.
fn leyenda(l: &mut Lienzo, y: f32, items: &[(&str, u32)]) {
    let mut x = MARGEN + UTIL;
    for (t, c) in items.iter().rev() {
        let w = ancho(t, F::Normal, 8.0);
        x -= w;
        l.texto(x, y, t, F::Normal, 8.0, TEXTO_2);
        x -= 11.0;
        l.rect_redondo(x, y - 0.5, 7.0, 7.0, 1.5, *c);
        x -= 12.0;
    }
}

/// Las etiquetas de los días bajo una gráfica.
fn rotulos_dias(l: &mut Lienzo, i: &Informe, x0: f32, hueco: f32, y: f32) {
    let z = zona(i);
    let n = i.dias.len();
    let cada = if n <= 10 { 1 } else { n.div_ceil(16) };
    for (k, d) in i.dias.iter().enumerate() {
        if k % cada != 0 {
            continue;
        }
        let f = en_zona(d.inicio, z);
        let t = if n <= 10 { format!("{} {}", DIAS_CORTOS[f.weekday().num_days_from_monday() as usize], f.day()) } else { f.day().to_string() };
        let w = ancho(&t, F::Normal, 7.5);
        l.texto(x0 + hueco * (k as f32 + 0.5) - w / 2.0, y, &t, F::Normal, 7.5, TEXTO_3);
    }
}

/// Gráfica de barras: una por día, apiladas (de abajo arriba) con sus colores.
fn barras(l: &mut Lienzo, i: &Informe, alto: f32, valores: &[Vec<(f64, u32)>], rotulo: &dyn Fn(f64) -> String) {
    let eje = 34.0;
    l.sitio(alto + 18.0);
    let (x0, w) = (MARGEN + eje, UTIL - eje);
    let base = l.y - alto;
    let max = tope(valores.iter().map(|v| v.iter().map(|(x, _)| *x).sum::<f64>()).fold(0.0, f64::max));
    // Rejilla: 0, la mitad y el tope.
    for k in 0..=2 {
        let y = base + alto * k as f32 / 2.0;
        if k == 0 {
            l.linea(x0, y, x0 + w, y, BORDE_FUERTE, 0.75, false);
        } else {
            l.linea(x0, y, x0 + w, y, BORDE, 0.5, true);
        }
        let t = rotulo(max * k as f64 / 2.0);
        l.texto_derecha(x0 - 6.0, y - 2.5, &t, F::Normal, 7.0, TEXTO_3);
    }
    let n = valores.len().max(1);
    let hueco = w / n as f32;
    let bw = (hueco * 0.62).min(26.0);
    for (k, v) in valores.iter().enumerate() {
        let mut y = base;
        let x = x0 + hueco * k as f32 + (hueco - bw) / 2.0;
        for (valor, color) in v {
            if *valor <= 0.0 {
                continue;
            }
            let h = (alto as f64 * valor / max) as f32;
            l.rect(x, y, bw, h.max(0.8), *color);
            y += h;
        }
    }
    rotulos_dias(l, i, x0, hueco, base - 11.0);
    l.y = base - 18.0;
}

fn grafica_copias(l: &mut Lienzo, i: &Informe) {
    l.seccion("Copias por día", 130.0);
    leyenda(l, l.y + 9.0, &[("Correctas", OK), ("Con avisos", WARN), ("Fallidas", BAD)]);
    let valores: Vec<Vec<(f64, u32)>> =
        i.dias.iter().map(|d| vec![(d.copias.correctas() as f64, OK), (d.copias.aviso as f64, WARN), (d.copias.fallo as f64, BAD)]).collect();
    barras(l, i, 100.0, &valores, &|x| numero(x.round() as u64));
}

fn grafica_datos(l: &mut Lienzo, i: &Informe) {
    l.seccion("Datos nuevos por día", 100.0);
    let mut t = format!("{} en total", tamano(i.espacio.nuevos));
    if let Some(x) = tendencia(i) {
        t.push_str(&format!(" · {x}"));
    }
    l.texto_derecha(MARGEN + UTIL, l.y + 9.0, &t, F::Normal, 8.0, TEXTO_2);
    let valores: Vec<Vec<(f64, u32)>> = i.dias.iter().map(|d| vec![(d.bytes as f64, MARCA)]).collect();
    barras(l, i, 70.0, &valores, &|x| if x <= 0.0 { "0".into() } else { tamano(x as u64) });
}

fn regla(l: &mut Lienzo, i: &Informe) {
    l.seccion("Regla 3-2-1-1-0", 80.0);
    let z = zona(i);
    let Some(r) = &i.regla else {
        l.parrafo(
            "Aún no hay datos de la regla para este cliente: la consola la calcula al abrir Informes. 3 copias de los datos, en 2 soportes distintos, 1 fuera del sitio, 1 inmutable o aislado y 0 errores al verificar y probar la restauración.",
            F::Normal,
            9.5,
            TEXTO_2,
        );
        return;
    };
    let t = format!(
        "{} de {} la cumplen entera (3 copias, 2 soportes, 1 fuera del sitio, 1 inmutable o aislado y 0 errores). Calculada en la consola el {}.",
        numero(r.cumplen as u64),
        plural(r.total as u64, "copia", "copias"),
        fecha_corta(r.calculada, z)
    );
    l.parrafo(&t, F::Normal, 9.5, TEXTO_2);
    l.y -= 6.0;
    let alto = 40.0;
    l.sitio(alto);
    let ancho = (UTIL - 4.0 * 6.0) / 5.0;
    for (k, nombre) in PARTES.iter().enumerate() {
        let x = MARGEN + k as f32 * (ancho + 6.0);
        l.rect_redondo(x, l.y - alto, ancho, alto, 5.0, SUPERFICIE_2);
        l.texto(x + 8.0, l.y - 14.0, &recortar(nombre, F::Negrita, 7.8, ancho - 12.0), F::Negrita, 7.8, TEXTO_1);
        let (texto, color) = match r.faltan[k] {
            0 => ("Cumplen todas".to_string(), OK),
            n => (format!("No la cumplen {}", plural(n as u64, "copia", "copias")).replace("No la cumplen 1 copia", "No la cumple 1 copia"), WARN),
        };
        l.circulo(x + 11.0, l.y - 28.5, 2.6, color);
        l.texto(x + 17.0, l.y - 31.0, &recortar(&texto, F::Normal, 7.5, ancho - 22.0), F::Normal, 7.5, color);
    }
    l.y -= alto + 10.0;
    if !r.sin_cumplir.is_empty() {
        let filas: Vec<Vec<Celda>> = r.sin_cumplir.iter().map(|c| vec![celda(&c.equipo), celda(&c.copia), celda(&c.falta)]).collect();
        l.tabla(&[col("EQUIPO", 120.0), col("COPIA", 130.0), col_2("QUÉ LE FALTA", UTIL - 250.0)], &filas);
        let mas = r.total.saturating_sub(r.cumplen) as usize;
        if mas > r.sin_cumplir.len() {
            l.y -= 4.0;
            l.parrafo(&format!("… y {} más en la consola.", numero((mas - r.sin_cumplir.len()) as u64)), F::Normal, 8.5, TEXTO_3);
        }
    }
}

fn pruebas(l: &mut Lienzo, i: &Informe) {
    l.seccion("Verificaciones, pruebas de restauración y restauraciones", 80.0);
    let z = zona(i);
    let fila = |nombre: &str, ok: u32, fallo: u32, extra: String| {
        let total = ok + fallo;
        vec![
            celda(nombre),
            if total == 0 { celda("Ninguna en el periodo") } else { Celda { punto: Some(OK), ..celda(plural(ok as u64, "correcta", "correctas")) } },
            if fallo == 0 {
                celda(if total == 0 { "" } else { "Ninguna fallida" })
            } else {
                Celda { punto: Some(BAD), color: Some(BAD), ..celda(plural(fallo as u64, "fallida", "fallidas")) }
            },
            celda(extra),
        ]
    };
    let r = &i.restauraciones;
    let mut extra_r = Vec::new();
    if r.bytes > 0 {
        extra_r.push(tamano(r.bytes));
    }
    if r.archivos > 0 {
        extra_r.push(plural(r.archivos, "archivo", "archivos"));
    }
    if let Some(d) = r.duracion_media_s {
        extra_r.push(format!("{} de media", duracion(d)));
    }
    let filas = vec![
        fila("Verificaciones", i.verificaciones.ok, i.verificaciones.fallo, String::new()),
        fila("Pruebas de restauración", i.pruebas.ok, i.pruebas.fallo, String::new()),
        fila("Restauraciones", r.ok, r.fallo, extra_r.join(" · ")),
    ];
    l.tabla(&[col("", 125.0), col("CORRECTAS", 95.0), col("FALLIDAS", 85.0), col("", UTIL - 305.0)], &filas);
    let fallidas: Vec<Vec<Celda>> = i
        .verificaciones
        .fallidas
        .iter()
        .map(|f| ("Verificación", f))
        .chain(i.pruebas.fallidas.iter().map(|f| ("Prueba", f)))
        .map(|(que, f)| vec![celda(que), celda(&f.equipo), celda(&f.repo), celda(fecha_corta(f.hora, z)), celda(&f.mensaje)])
        .collect();
    if !fallidas.is_empty() {
        l.y -= 12.0;
        l.sitio(40.0);
        l.texto(MARGEN, l.y - 9.5, "Las que fallaron", F::Negrita, 9.5, TEXTO_1);
        l.y -= 16.0;
        l.tabla(&[col("", 70.0), col("EQUIPO", 95.0), col("REPOSITORIO", 100.0), col("CUÁNDO", 70.0), col_2("MOTIVO", UTIL - 335.0)], &fallidas);
    }
}

fn equipos(l: &mut Lienzo, i: &Informe) {
    l.seccion("Equipos", 60.0);
    let z = zona(i);
    let filas: Vec<Vec<Celda>> = i
        .equipos
        .iter()
        .map(|e| {
            let (estado, color) = estado_equipo(&e.estado);
            let c: &Cuenta = &e.copias;
            vec![
                Celda { negrita: true, ..celda(&e.nombre) },
                Celda { punto: Some(color), ..celda(estado) },
                celda(numero(c.total() as u64)),
                celda(numero(c.correctas() as u64)),
                Celda { color: (c.fallo > 0).then_some(BAD), ..celda(numero(c.fallo as u64)) },
                celda(e.ultima_ok.map(|t| fecha_corta(t, z)).unwrap_or_else(|| "—".into())),
                celda(e.bytes.map(tamano).unwrap_or_else(|| "—".into())),
            ]
        })
        .collect();
    if filas.is_empty() {
        l.parrafo("Este cliente aún no tiene equipos con copias.", F::Normal, 9.5, TEXTO_2);
        return;
    }
    l.tabla(
        &[
            col("EQUIPO", 140.0),
            col("HOY", 78.0),
            col_d("COPIAS", 48.0),
            col_d("CORRECTAS", 58.0),
            col_d("FALLIDAS", 52.0),
            col("ÚLTIMA CORRECTA", 70.0),
            col_d("ESPACIO", UTIL - 446.0),
        ],
        &filas,
    );
    if i.equipos_mas > 0 {
        l.y -= 4.0;
        l.parrafo(&format!("… y {} más en la consola.", plural(i.equipos_mas as u64, "equipo", "equipos")), F::Normal, 8.5, TEXTO_3);
    }
}

fn fallos(l: &mut Lienzo, i: &Informe) {
    if i.fallos.is_empty() {
        return;
    }
    l.seccion("Copias que fallaron", 60.0);
    let z = zona(i);
    let filas: Vec<Vec<Celda>> = i
        .fallos
        .iter()
        .map(|f| {
            vec![
                Celda { negrita: true, ..celda(&f.equipo) },
                celda(&f.copia),
                Celda { color: Some(BAD), ..celda(plural(f.veces as u64, "vez", "veces")) },
                celda(fecha_corta(f.ultima, z)),
                celda(if f.mensaje.is_empty() { "—".to_string() } else { f.mensaje.clone() }),
            ]
        })
        .collect();
    l.tabla(&[col("EQUIPO", 105.0), col("COPIA", 105.0), col("VECES", 50.0), col("LA ÚLTIMA", 70.0), col_2("MOTIVO", UTIL - 330.0)], &filas);
    if i.fallos_mas > 0 {
        l.y -= 4.0;
        l.parrafo(&format!("… y {} más en la consola.", plural(i.fallos_mas as u64, "copia", "copias")), F::Normal, 8.5, TEXTO_3);
    }
}

fn avisos(l: &mut Lienzo, i: &Informe) {
    l.seccion("Avisos importantes sin resolver", 40.0);
    let z = zona(i);
    if i.avisos.is_empty() {
        l.parrafo("Ninguno: no queda ningún aviso crítico ni importante abierto.", F::Normal, 9.5, TEXTO_2);
        return;
    }
    let filas: Vec<Vec<Celda>> = i
        .avisos
        .iter()
        .map(|a| {
            let color = if a.severidad == Severidad::Critico { BAD } else { WARN };
            let veces = if a.veces > 1 {
                format!("{} desde el {}", plural(a.veces as u64, "vez", "veces"), fecha_corta(a.desde, z))
            } else {
                format!("desde el {}", fecha_corta(a.desde, z))
            };
            vec![Celda { punto: Some(color), color: Some(color), ..celda(a.severidad.texto()) }, celda(&a.titulo), celda(veces)]
        })
        .collect();
    l.tabla(&[col("", 82.0), col_2("AVISO", UTIL - 82.0 - 140.0), col("", 140.0)], &filas);
}

/// El pie de cada página: de quién es, la página y lo que nunca lleva.
fn pies(l: &mut Lienzo, i: &Informe) {
    let n = l.paginas.len();
    let izquierda = format!(
        "{} · Informe {} · {}",
        i.cliente,
        match i.periodo {
            Periodo::Semanal => "semanal",
            Periodo::Mensual => "mensual",
        },
        texto_periodo(i)
    );
    let quien = match &i.prestador {
        Some(p) => format!("Preparado por {p} con Resguardo. No lleva contraseñas, claves ni nombres de archivos."),
        None => "Preparado con Resguardo. No lleva contraseñas, claves ni nombres de archivos.".to_string(),
    };
    for k in 0..n {
        l.actual = k;
        l.linea(MARGEN, 40.0, MARGEN + UTIL, 40.0, BORDE, 0.5, false);
        l.texto(MARGEN, 28.0, &recortar(&izquierda, F::Normal, 7.5, UTIL - 80.0), F::Normal, 7.5, TEXTO_3);
        l.texto_derecha(MARGEN + UTIL, 28.0, &format!("Página {} de {n}", k + 1), F::Normal, 7.5, TEXTO_3);
        l.texto(MARGEN, 18.0, &recortar(&quien, F::Normal, 7.0, UTIL), F::Normal, 7.0, TEXTO_3);
    }
}

/// Las páginas del informe y lo que llevan escrito (sin el PDF alrededor).
fn componer(i: &Informe, logo: Option<&Imagen>) -> Lienzo {
    let mut l = Lienzo::nuevo();
    let medidas = logo.map(|im| {
        // Como mucho 160 × 36 pt, con su proporción.
        let (w, h) = (im.ancho as f32, im.alto as f32);
        let k = (160.0 / w).min(36.0 / h);
        (w * k, h * k)
    });
    cabecera(&mut l, i, medidas);
    cifras(&mut l, i);
    grafica_copias(&mut l, i);
    grafica_datos(&mut l, i);
    regla(&mut l, i);
    pruebas(&mut l, i);
    equipos(&mut l, i);
    fallos(&mut l, i);
    avisos(&mut l, i);
    pies(&mut l, i);
    l
}

/// El PDF del informe. `logo`: el PNG que lleva arriba (si no se puede leer, sin él).
pub fn pdf(i: &Informe, logo: Option<&[u8]>) -> Vec<u8> {
    let imagen = logo.and_then(imagen);
    let l = componer(i, imagen.as_ref());

    let mut pdf = Pdf::new();
    let catalogo = Ref::new(1);
    let arbol = Ref::new(2);
    let (normal, negrita) = (Ref::new(3), Ref::new(4));
    let info = Ref::new(5);
    let (r_logo, r_alfa) = (Ref::new(6), Ref::new(7));
    let paginas: Vec<(Ref, Ref)> = (0..l.paginas.len() as i32).map(|k| (Ref::new(10 + 2 * k), Ref::new(11 + 2 * k))).collect();

    pdf.catalog(catalogo).pages(arbol);
    pdf.pages(arbol).kids(paginas.iter().map(|(p, _)| *p)).count(paginas.len() as i32);
    for (p, contenido) in &paginas {
        let mut pg = pdf.page(*p);
        pg.media_box(Rect::new(0.0, 0.0, ANCHO, ALTO));
        pg.parent(arbol);
        pg.contents(*contenido);
        let mut recursos = pg.resources();
        recursos.fonts().pair(F::Normal.nombre(), normal).pair(F::Negrita.nombre(), negrita);
        if imagen.is_some() {
            recursos.x_objects().pair(Name(b"L"), r_logo);
        }
        recursos.finish();
        pg.finish();
    }
    pdf.type1_font(normal).base_font(Name(b"Helvetica")).encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.type1_font(negrita).base_font(Name(b"Helvetica-Bold")).encoding_predefined(Name(b"WinAnsiEncoding"));
    if let Some(im) = &imagen {
        let mut x = pdf.image_xobject(r_logo, &im.rgb);
        x.filter(Filter::FlateDecode);
        x.width(im.ancho as i32);
        x.height(im.alto as i32);
        x.color_space().device_rgb();
        x.bits_per_component(8);
        if im.alfa.is_some() {
            x.s_mask(r_alfa);
        }
        x.finish();
        if let Some(a) = &im.alfa {
            let mut m = pdf.image_xobject(r_alfa, a);
            m.filter(Filter::FlateDecode);
            m.width(im.ancho as i32);
            m.height(im.alto as i32);
            m.color_space().device_gray();
            m.bits_per_component(8);
            m.finish();
        }
    }
    for ((_, contenido), c) in paginas.iter().zip(l.paginas) {
        let crudo = c.finish();
        let comprimido = compress_to_vec_zlib(&crudo, 6);
        pdf.stream(*contenido, &comprimido).filter(Filter::FlateDecode);
    }
    let titulo = format!("Informe de copias · {} · {}", i.cliente, texto_periodo(i));
    pdf.document_info(info).title(TextStr(&titulo)).creator(TextStr("Resguardo Server")).producer(TextStr("Resguardo Server"));
    pdf.finish()
}

/// Todo lo que lleva escrito el informe (para las pruebas).
#[cfg(test)]
pub fn textos(i: &Informe, logo: Option<&[u8]>) -> (usize, Vec<String>) {
    let imagen = logo.and_then(imagen);
    let l = componer(i, imagen.as_ref());
    (l.paginas.len(), l.escrito)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texto_y_anchos() {
        assert_eq!(win("Año ñ «x» ·—…€"), vec![b'A', 0xf1, b'o', b' ', 0xf1, b' ', 0xab, b'x', 0xbb, b' ', 0xb7, 0x97, 0x85, 0x80]);
        assert_eq!(win("✓ 漢"), b"? ?".to_vec());
        // «Hola» en Helvetica 10 pt: H 722 + o 556 + l 222 + a 556 = 2056 → 20,56 pt.
        assert!((ancho("Hola", F::Normal, 10.0) - 20.56).abs() < 0.01);
        assert!(ancho("Hola", F::Negrita, 10.0) > ancho("Hola", F::Normal, 10.0));
        assert_eq!(recortar("Un nombre de equipo muy largo", F::Normal, 10.0, 60.0).chars().last(), Some('…'));
        assert!(ancho(&recortar("Un nombre de equipo muy largo", F::Normal, 10.0, 60.0), F::Normal, 10.0) <= 60.0);
        let ls = lineas("una frase bastante larga que no cabe en una sola línea", F::Normal, 10.0, 100.0);
        assert!(ls.len() > 1 && ls.iter().all(|l| ancho(l, F::Normal, 10.0) <= 100.0), "{ls:?}");
        assert_eq!(numero(1_234_567), "1.234.567");
        assert_eq!(numero(12), "12");
        assert_eq!(porcentaje(97.0), "97\u{a0}%");
        assert_eq!(porcentaje(97.44), "97,4\u{a0}%");
        assert_eq!(porcentaje(99.97), "99,9\u{a0}%");
        assert_eq!(porcentaje(100.0), "100\u{a0}%");
        assert_eq!((duracion(35), duracion(250), duracion(4320)), ("35 s".into(), "4 min".into(), "1 h 12 min".into()));
        assert_eq!((tope(0.0), tope(3.0), tope(7.0), tope(23.0), tope(180.0)), (1.0, 4.0, 10.0, 40.0, 200.0));
    }
}
