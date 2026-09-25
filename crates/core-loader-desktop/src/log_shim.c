/* Ponte do `retro_log_printf_t` (variádico, estilo printf) pro Rust: Rust
 * estável não define função C variádica, então o C formata a mensagem e
 * entrega a string pronta em `reemu_core_log` (src/ffi_state.rs). */
#include <stdarg.h>
#include <stdio.h>

extern void reemu_core_log(unsigned level, const char *msg);

void reemu_log_printf(unsigned level, const char *fmt, ...) {
    char buf[1024];
    va_list ap;
    if (!fmt) return;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof buf, fmt, ap);
    va_end(ap);
    reemu_core_log(level, buf);
}
