# Política de Seguridade de OPENVIDEOMIX

A seguridade, a integridade dos sinais en directo e a privacidade dos participantes son prioritarias no desenvolvemento de OPENVIDEOMIX.

---

## 1. Versións Soportadas

Actualmente prestamos soporte de seguridade activo unicamente sobre a rama principal de desenvolvemento:

| Versión | Soportada |
| --- | --- |
| `main` (desenvolvemento activo) | ✅ Si |
| `< 0.1.0` (versións históricas) | ❌ Non |

---

## 2. Seguridade na API de Control Local

1. **Illamento de Rede**:
   - A API local de control (`local-control-api`) só escoita de xeito predeterminado en `127.0.0.1:21873` (localhost).
   - Non se expón automaticamente a redes locais (LAN) nin a internet sen configuración explícita.
2. **Autenticación Obrigatoria**:
   - Requírese a variable de contorna `OPENVIDEOMIX_CONTROL_TOKEN` (de 32 a 256 caracteres ASCII) mediante cabeceira `Authorization: Bearer <TOKEN>`.
   - Rexeitaranse peticións con cabeceiras `Origin` de navegadores web para mitigar ataques CSRF / WebSockets dende sitios web maliciosos.
3. **Credenciais e Segredos**:
   - Nunca fagas commit de tokens de autenticación nin de credenciais de streaming RTMP no repositorio de Git.

---

## 3. Privacidade e Datos Biométricos

- As funcións futuras de autorrealización multimodal (#47) procesan audio e seguimento de labios **100% en local** (sen enviar imaxes, vídeos nin voz a APIs na nube de terceiros).
- Os datos de calibración almacénanse en local e requiren o consentimento dos participantes.

---

## 4. Como Notificar unha Vulnerabilidade

Se atopas unha vulnerabilidade de seguridade en OPENVIDEOMIX, agradecemos que a reportes de forma confidencial:

1. **Non crees unha Issue pública en GitHub.**
2. Envía un correo electrónico detallado a `dodemo@users.noreply.github.com` co asunto:  
   `[SECURITY VULNERABILITY] Descrición breve`
3. Inclúe os pasos exactos para reproducir a vulnerabilidade e o impacto potencial.
4. Responderemos nun prazo máximo de 48 horas e traballaremos nun parche antes da publicación aberta.
