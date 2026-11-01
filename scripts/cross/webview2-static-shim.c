#include <windows.h>
#include <limits.h>
#include <stdlib.h>

void *nd_new_nothrow(size_t n, const void *tag) __asm__("??2@YAPEAX_KAEBUnothrow_t@std@@@Z");
void *nd_new_nothrow(size_t n, const void *tag) { (void)tag; return malloc(n ? n : 1); }
void nd_delete_sized(void *p, size_t n) __asm__("??3@YAXPEAX_K@Z");
void nd_delete_sized(void *p, size_t n) { (void)n; free(p); }
void *nd_new_array(size_t n) __asm__("??_U@YAPEAX_K@Z");
void *nd_new_array(size_t n) { void *p = malloc(n ? n : 1); if (!p) abort(); return p; }
void nd_delete_array(void *p) __asm__("??_V@YAXPEAX@Z");
void nd_delete_array(void *p) { free(p); }
const char nd_std_nothrow __asm__("?nothrow@std@@3Unothrow_t@1@B") = 0;

#define ND_UNINITIALIZED 0
#define ND_BEING_INITIALIZED (-1)
int _Init_global_epoch = INT_MIN;
__thread int _Init_thread_epoch = INT_MIN;
static SRWLOCK nd_init_lock = SRWLOCK_INIT;
static CONDITION_VARIABLE nd_init_cv = CONDITION_VARIABLE_INIT;

void _Init_thread_header(int *once) {
    AcquireSRWLockExclusive(&nd_init_lock);
    if (*once == ND_UNINITIALIZED) {
        *once = ND_BEING_INITIALIZED;
    } else {
        while (*once == ND_BEING_INITIALIZED) {
            SleepConditionVariableSRW(&nd_init_cv, &nd_init_lock, 100, 0);
            if (*once == ND_UNINITIALIZED) {
                *once = ND_BEING_INITIALIZED;
                ReleaseSRWLockExclusive(&nd_init_lock);
                return;
            }
        }
        _Init_thread_epoch = _Init_global_epoch;
    }
    ReleaseSRWLockExclusive(&nd_init_lock);
}

void _Init_thread_footer(int *once) {
    AcquireSRWLockExclusive(&nd_init_lock);
    ++_Init_global_epoch;
    *once = _Init_global_epoch;
    _Init_thread_epoch = _Init_global_epoch;
    ReleaseSRWLockExclusive(&nd_init_lock);
    WakeAllConditionVariable(&nd_init_cv);
}

void _Init_thread_abort(int *once) {
    AcquireSRWLockExclusive(&nd_init_lock);
    *once = ND_UNINITIALIZED;
    ReleaseSRWLockExclusive(&nd_init_lock);
    WakeAllConditionVariable(&nd_init_cv);
}

__asm__(".text\n"
        ".globl nd_guard_dispatch_icall_nop\n"
        "nd_guard_dispatch_icall_nop:\n"
        "    jmp *%rax\n");
extern void nd_guard_dispatch_icall_nop(void);
__attribute__((weak)) void *__guard_dispatch_icall_fptr = (void *)&nd_guard_dispatch_icall_nop;

__attribute__((weak)) int _purecall(void) { abort(); }

__attribute__((weak)) UINT_PTR __security_cookie = 0x00002B992DDFA232ULL;

__attribute__((weak)) void __security_check_cookie(UINT_PTR cookie) {
    if (cookie != __security_cookie) __fastfail(2);
}

__attribute__((constructor)) static void nd_security_init_cookie(void) {
    LARGE_INTEGER pc;
    FILETIME ft;
    QueryPerformanceCounter(&pc);
    GetSystemTimeAsFileTime(&ft);
    UINT_PTR c = (UINT_PTR)pc.QuadPart ^ ((UINT_PTR)ft.dwHighDateTime << 32 | ft.dwLowDateTime)
               ^ (UINT_PTR)GetCurrentProcessId() << 16 ^ (UINT_PTR)GetCurrentThreadId()
               ^ (UINT_PTR)&c;
    c &= 0x0000FFFFFFFFFFFFULL;
    if (c == 0 || c == 0x00002B992DDFA232ULL) c = 0x00002B992DDFA233ULL;
    __security_cookie = c;
}
