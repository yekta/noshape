// Windows crash guard (WIN-CRASH-GUARD): the POSIX guard (common.cpp) turns
// a signal raised inside a bridge call into that call's error; on Windows a
// crash is a structured exception, caught here by __try / __except around
// the call (noshape_occt.hpp `seh_run`). Kept in its own file: <windows.h>
// must not meet OCCT's headers, and a function with __try may hold no C++
// objects that need unwinding. MSVC only; empty elsewhere.
#if defined(_WIN32) && defined(_MSC_VER)
  #define WIN32_LEAN_AND_MEAN
  #define NOMINMAX
  #include <windows.h>
  #include <csetjmp>
  #include <cstdlib>
  #include <exception>

namespace noshape {
namespace {
// Guarded calls running on this thread.
thread_local int seh_depth = 0;
// std::terminate inside a guarded call (OCCT 8 throws from noexcept code:
// gp_Dir of a zero vector) jumps back to the call, which reports this code.
// (A structured exception raised from the handler can't get out: the C
// runtime's terminate() runs the handler under its own __try / __except
// and then aborts, 0xc0000409 on Windows CI.)
constexpr DWORD          terminate_code = 0xE0A50001;
thread_local std::jmp_buf* terminate_jmp = nullptr;
// The terminate handler is per thread in MSVC's runtime.
thread_local bool                   terminate_installed = false;
thread_local std::terminate_handler previous_terminate  = nullptr;

int crash_filter(DWORD code)
{
  switch (code)
  {
    case EXCEPTION_ACCESS_VIOLATION:
    case EXCEPTION_IN_PAGE_ERROR:
    case EXCEPTION_ILLEGAL_INSTRUCTION:
    case EXCEPTION_PRIV_INSTRUCTION:
    case EXCEPTION_INT_DIVIDE_BY_ZERO:
    case EXCEPTION_INT_OVERFLOW:
    case EXCEPTION_ARRAY_BOUNDS_EXCEEDED:
    case EXCEPTION_DATATYPE_MISALIGNMENT:
    case EXCEPTION_FLT_DIVIDE_BY_ZERO:
    case EXCEPTION_FLT_INVALID_OPERATION:
    case EXCEPTION_FLT_OVERFLOW:
    case EXCEPTION_FLT_UNDERFLOW:
    case EXCEPTION_FLT_STACK_CHECK:
      return EXCEPTION_EXECUTE_HANDLER;
    default:
      // C++ exceptions (caught inside the call) and the rest go on.
      return EXCEPTION_CONTINUE_SEARCH;
  }
}

void on_terminate()
{
  if (seh_depth > 0 && terminate_jmp != nullptr)
    std::longjmp(*terminate_jmp, 1);
  if (previous_terminate != nullptr)
    previous_terminate();
  std::abort();
}

// No C++ objects in here (C2712).
unsigned long seh_frame(void (*fn)(void*), void* ctx)
{
  __try
  {
    fn(ctx);
  }
  __except (crash_filter(GetExceptionCode()))
  {
    return GetExceptionCode();
  }
  return 0;
}
} // namespace

// No C++ objects in here either (longjmp lands in this frame).
unsigned long seh_call(void (*fn)(void*), void* ctx) noexcept
{
  if (!terminate_installed)
  {
    previous_terminate  = std::set_terminate(on_terminate);
    terminate_installed = true;
  }
  std::jmp_buf               env;
  std::jmp_buf* const        outer = terminate_jmp;
  if (setjmp(env) != 0)
  {
    terminate_jmp = outer;
    --seh_depth;
    return terminate_code;
  }
  terminate_jmp = &env;
  ++seh_depth;
  const unsigned long code = seh_frame(fn, ctx);
  --seh_depth;
  terminate_jmp = outer;
  return code;
}
} // namespace noshape
#endif
