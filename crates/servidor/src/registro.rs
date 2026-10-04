//! Líneas del registro pensadas para fail2ban (packaging/linux/fail2ban): una
//! por acceso fallido, con la IP real de quien lo intentó (detrás de un proxy,
//! la de `X-Forwarded-For`, ver `ip_real`). Formato fijo, sin datos de nadie:
//!
//! ```text
//! Acceso fallido desde 203.0.113.5: entrar (POST /api/sesion)
//! Límite de intentos superado desde 203.0.113.5 (POST /api/sesion)
//! ```
//!
//! fail2ban solo cuenta las de «Acceso fallido»: un límite superado lo puede
//! provocar alguien legítimo con prisa (y ya lo frena el propio servidor).

use std::net::IpAddr;

/// La línea de un acceso fallido (la que busca el filtro de fail2ban).
pub fn linea_acceso_fallido(ip: IpAddr, que: &str, metodo: &str, ruta: &str) -> String {
    format!("Acceso fallido desde {ip}: {que} ({metodo} {})", ruta_corta(ruta))
}

/// La línea de un límite de intentos superado.
pub fn linea_limite(ip: IpAddr, metodo: &str, ruta: &str) -> String {
    format!("Límite de intentos superado desde {ip} ({metodo} {})", ruta_corta(ruta))
}

/// La ruta sin la consulta y sin caracteres de control (no se mete nada raro en el registro).
fn ruta_corta(ruta: &str) -> String {
    ruta.split('?').next().unwrap_or("").chars().filter(|c| !c.is_control()).take(120).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn formato_fijo_para_fail2ban() {
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        assert_eq!(linea_acceso_fallido(ip, "entrar", "POST", "/api/sesion"), "Acceso fallido desde 203.0.113.5: entrar (POST /api/sesion)");
        let v6: IpAddr = "2001:db8::7".parse().unwrap();
        assert_eq!(linea_acceso_fallido(v6, "equipo", "GET", "/api/agente/canal?reto=x\n"), "Acceso fallido desde 2001:db8::7: equipo (GET /api/agente/canal)");
        assert_eq!(linea_limite(ip, "POST", "/api/inicio"), "Límite de intentos superado desde 203.0.113.5 (POST /api/inicio)");
        // Lo que busca el filtro de packaging/linux/fail2ban/filter.d/resguardo-server.conf.
        let filtro = include_str!("../../../packaging/linux/fail2ban/filter.d/resguardo-server.conf");
        assert!(filtro.contains(r"Acceso fallido desde <HOST>:"), "el filtro de fail2ban tiene que buscar esta línea");
    }
}
