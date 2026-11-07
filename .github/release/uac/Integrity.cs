// A process's integrity level (Medium for a UAC-filtered admin, High once elevated), for update_across_uac.ps1.
using System;
using System.Runtime.InteropServices;

public static class Integrity {
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool OpenProcessToken(IntPtr p, uint access, out IntPtr t);
    [DllImport("advapi32.dll", SetLastError = true)] static extern bool GetTokenInformation(IntPtr t, int cls, IntPtr buf, int len, out int ret);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthority(IntPtr sid, uint n);
    [DllImport("advapi32.dll")] static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);

    public static string Of(int pid) {
        IntPtr p = OpenProcess(0x1000, false, pid);
        if (p == IntPtr.Zero) return "?";
        IntPtr t = IntPtr.Zero, buf = IntPtr.Zero;
        try {
            if (!OpenProcessToken(p, 0x0008, out t)) return "?";
            int n;
            GetTokenInformation(t, 25, IntPtr.Zero, 0, out n);
            buf = Marshal.AllocHGlobal(n);
            if (!GetTokenInformation(t, 25, buf, n, out n)) return "?";
            IntPtr sid = Marshal.ReadIntPtr(buf);
            int rid = Marshal.ReadInt32(GetSidSubAuthority(sid, (uint)(Marshal.ReadByte(GetSidSubAuthorityCount(sid)) - 1)));
            return rid >= 0x4000 ? "System" : rid >= 0x3000 ? "High" : rid >= 0x2000 ? "Medium" : "Low";
        } finally {
            if (buf != IntPtr.Zero) Marshal.FreeHGlobal(buf);
            if (t != IntPtr.Zero) CloseHandle(t);
            CloseHandle(p);
        }
    }
}
