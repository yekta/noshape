# Windows check of the graphics-area cursors (crates/app/src/cursor.rs).
# The smoke run writes every cursor image as a .cur file (NOSHAPE_CURSOR_DUMP);
# this loads each one through Windows twice, with LoadImage (the file as
# Windows reads it) and with CreateIconIndirect from the same pixels (what
# cursor_windows.rs does), reads the hot spots back with GetIconInfo, draws
# both with DrawIconEx on white and on grey, and compares Windows' drawing on
# white with the file's pixels composited on white.
# Output in OutDir: cursors-sheet.png, one <name>.png per cursor (drawn on
# white at 1:1), cursors.txt (plain GDI and a small PNG writer, so no
# System.Drawing version issues). A hot spot that reads back differently or a
# drawing more than 2 levels off is reported as a workflow error.
param([Parameter(Mandatory)][string]$CurDir, [Parameter(Mandatory)][string]$OutDir)
$ErrorActionPreference = 'Stop'
Add-Type -ReferencedAssemblies System.Runtime, System.Collections, System.Console, System.Runtime.InteropServices, System.IO.Compression -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Runtime.InteropServices;

public static class CursorCheck {
    [StructLayout(LayoutKind.Sequential)]
    struct ICONINFO { public bool fIcon; public int xHotspot; public int yHotspot; public IntPtr hbmMask; public IntPtr hbmColor; }
    [StructLayout(LayoutKind.Sequential)]
    struct BITMAPINFOHEADER { public uint biSize; public int biWidth; public int biHeight; public ushort biPlanes; public ushort biBitCount; public uint biCompression; public uint biSizeImage; public int biXPelsPerMeter; public int biYPelsPerMeter; public uint biClrUsed; public uint biClrImportant; }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr LoadImage(IntPtr hinst, string name, uint type, int cx, int cy, uint load);
    [DllImport("user32.dll", SetLastError = true)]
    static extern IntPtr CreateIconIndirect(ref ICONINFO info);
    [DllImport("user32.dll")]
    static extern bool GetIconInfo(IntPtr icon, out ICONINFO info);
    [DllImport("user32.dll")]
    static extern bool DrawIconEx(IntPtr hdc, int x, int y, IntPtr icon, int cx, int cy, uint step, IntPtr brush, uint flags);
    [DllImport("user32.dll")]
    static extern bool DestroyCursor(IntPtr cursor);
    [DllImport("gdi32.dll")]
    static extern IntPtr CreateDIBSection(IntPtr hdc, ref BITMAPINFOHEADER bmi, uint usage, out IntPtr bits, IntPtr section, uint offset);
    [DllImport("gdi32.dll")]
    static extern IntPtr CreateBitmap(int w, int h, uint planes, uint bpp, IntPtr bits);
    [DllImport("gdi32.dll")]
    static extern IntPtr CreateCompatibleDC(IntPtr hdc);
    [DllImport("gdi32.dll")]
    static extern IntPtr SelectObject(IntPtr hdc, IntPtr obj);
    [DllImport("gdi32.dll")]
    static extern bool DeleteDC(IntPtr hdc);
    [DllImport("gdi32.dll")]
    static extern bool GdiFlush();
    [DllImport("gdi32.dll")]
    static extern bool DeleteObject(IntPtr obj);

    const uint IMAGE_CURSOR = 2, LR_LOADFROMFILE = 0x10, DI_NORMAL = 3;

    // A top-down 32-bpp DIB (BGRA rows); bits returned.
    static IntPtr Dib(int w, int h, out IntPtr bits) {
        var bmi = new BITMAPINFOHEADER { biSize = 40, biWidth = w, biHeight = -h, biPlanes = 1, biBitCount = 32 };
        return CreateDIBSection(IntPtr.Zero, ref bmi, 0, out bits, IntPtr.Zero, 0);
    }

    // The hot spot of a cursor handle, read back from Windows.
    static int[] Hot(IntPtr cursor) {
        ICONINFO info;
        if (cursor == IntPtr.Zero || !GetIconInfo(cursor, out info)) return new[] { -1, -1 };
        if (info.hbmMask != IntPtr.Zero) DeleteObject(info.hbmMask);
        if (info.hbmColor != IntPtr.Zero) DeleteObject(info.hbmColor);
        return new[] { info.xHotspot, info.yHotspot };
    }

    // What cursor_windows.rs `make` does: a top-down 32-bpp DIB with the
    // straight-alpha BGRA rows, an empty mask, CreateIconIndirect.
    static IntPtr Indirect(byte[] topDown, int side, int hx, int hy) {
        IntPtr bits;
        IntPtr color = Dib(side, side, out bits);
        if (color == IntPtr.Zero || bits == IntPtr.Zero) return IntPtr.Zero;
        Marshal.Copy(topDown, 0, bits, topDown.Length);
        IntPtr mask = CreateBitmap(side, side, 1, 1, IntPtr.Zero);
        var info = new ICONINFO { fIcon = false, xHotspot = hx, yHotspot = hy, hbmMask = mask, hbmColor = color };
        IntPtr cursor = CreateIconIndirect(ref info);
        DeleteObject(color);
        DeleteObject(mask);
        return cursor;
    }

    // `cursor` drawn by Windows (DrawIconEx) on a w x h canvas of `bg` at
    // (x, y); returns the canvas as BGRA rows.
    sealed class Canvas : IDisposable {
        public readonly int W, H;
        readonly IntPtr dc, bmp, old, bits;
        public Canvas(int w, int h) {
            W = w; H = h;
            dc = CreateCompatibleDC(IntPtr.Zero);
            IntPtr b;
            bmp = Dib(w, h, out b);
            bits = b;
            old = SelectObject(dc, bmp);
        }
        public void Fill(int x, int y, int w, int h, byte grey) {
            var row = new byte[w * 4];
            for (int i = 0; i < w; i++) { row[i * 4] = grey; row[i * 4 + 1] = grey; row[i * 4 + 2] = grey; row[i * 4 + 3] = 255; }
            GdiFlush();
            for (int r = y; r < y + h; r++) Marshal.Copy(row, 0, bits + (r * W + x) * 4, w * 4);
        }
        public void Draw(IntPtr cursor, int x, int y) { DrawIconEx(dc, x, y, cursor, 0, 0, 0, IntPtr.Zero, DI_NORMAL); }
        public byte[] Pixels() { GdiFlush(); var p = new byte[W * H * 4]; Marshal.Copy(bits, p, 0, p.Length); return p; }
        public void Dispose() { SelectObject(dc, old); DeleteObject(bmp); DeleteDC(dc); }
    }

    static uint[] crcTable;
    static uint Crc(byte[] type, byte[] data) {
        if (crcTable == null) {
            crcTable = new uint[256];
            for (uint n = 0; n < 256; n++) { uint c = n; for (int k = 0; k < 8; k++) c = (c & 1) != 0 ? 0xEDB88320u ^ (c >> 1) : c >> 1; crcTable[n] = c; }
        }
        uint crc = 0xFFFFFFFFu;
        foreach (var arr in new[] { type, data }) foreach (byte x in arr) crc = crcTable[(crc ^ x) & 0xFF] ^ (crc >> 8);
        return crc ^ 0xFFFFFFFFu;
    }
    static void Chunk(Stream s, string type, byte[] data) {
        var t = System.Text.Encoding.ASCII.GetBytes(type);
        var len = BitConverter.GetBytes(data.Length); Array.Reverse(len); s.Write(len, 0, 4);
        s.Write(t, 0, 4); s.Write(data, 0, data.Length);
        var c = BitConverter.GetBytes(Crc(t, data)); Array.Reverse(c); s.Write(c, 0, 4);
    }
    // BGRA rows -> an RGB PNG.
    static void Png(string path, byte[] bgra, int w, int h) {
        var raw = new MemoryStream();
        using (var z = new ZLibStream(raw, CompressionLevel.Optimal, true)) {
            var row = new byte[1 + w * 3];
            for (int y = 0; y < h; y++) {
                for (int x = 0; x < w; x++) { int o = (y * w + x) * 4; row[1 + x * 3] = bgra[o + 2]; row[2 + x * 3] = bgra[o + 1]; row[3 + x * 3] = bgra[o]; }
                z.Write(row, 0, row.Length);
            }
        }
        var ihdr = new byte[13];
        var wb = BitConverter.GetBytes(w); Array.Reverse(wb); Buffer.BlockCopy(wb, 0, ihdr, 0, 4);
        var hb = BitConverter.GetBytes(h); Array.Reverse(hb); Buffer.BlockCopy(hb, 0, ihdr, 4, 4);
        ihdr[8] = 8; ihdr[9] = 2;
        using (var f = File.Create(path)) {
            f.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 }, 0, 8);
            Chunk(f, "IHDR", ihdr);
            Chunk(f, "IDAT", raw.ToArray());
            Chunk(f, "IEND", new byte[0]);
        }
    }

    // Returns the number of problems found.
    public static int Run(string curDir, string outDir) {
        Directory.CreateDirectory(outDir);
        var files = Directory.GetFiles(curDir, "*.cur");
        Array.Sort(files, StringComparer.Ordinal);
        const int cell = 80;
        var report = new List<string>();
        report.Add("row N of cursors-sheet.png = line N below; columns: LoadImage on white, on grey; CreateIconIndirect on white, on grey");
        int problems = 0;
        using (var sheet = new Canvas(4 * cell, Math.Max(1, files.Length) * cell)) {
            sheet.Fill(0, 0, sheet.W, sheet.H, 255);
            for (int i = 0; i < files.Length; i++) {
                string name = Path.GetFileNameWithoutExtension(files[i]);
                byte[] f = File.ReadAllBytes(files[i]);
                int side = f[6] == 0 ? 256 : f[6];
                int fhx = BitConverter.ToUInt16(f, 10), fhy = BitConverter.ToUInt16(f, 12);
                int px = BitConverter.ToInt32(f, 18) + 40;
                // The file's rows are bottom-up.
                var topDown = new byte[side * side * 4];
                for (int r = 0; r < side; r++) Buffer.BlockCopy(f, px + (side - 1 - r) * side * 4, topDown, r * side * 4, side * 4);

                IntPtr a = LoadImage(IntPtr.Zero, files[i], IMAGE_CURSOR, 0, 0, LR_LOADFROMFILE);
                IntPtr b = Indirect(topDown, side, fhx, fhy);
                int[] ha = Hot(a), hb = Hot(b);
                int y = i * cell;
                sheet.Fill(cell, y, cell, cell, 128);
                sheet.Fill(3 * cell, y, cell, cell, 128);
                if (a != IntPtr.Zero) { sheet.Draw(a, 4, y + 4); sheet.Draw(a, cell + 4, y + 4); }
                if (b != IntPtr.Zero) { sheet.Draw(b, 2 * cell + 4, y + 4); sheet.Draw(b, 3 * cell + 4, y + 4); }

                // CreateIconIndirect on white vs the file's pixels over white.
                int worst = -1;
                if (b != IntPtr.Zero) {
                    using (var one = new Canvas(side, side)) {
                        one.Fill(0, 0, side, side, 255);
                        one.Draw(b, 0, 0);
                        var got = one.Pixels();
                        Png(Path.Combine(outDir, name + ".png"), got, side, side);
                        worst = 0;
                        for (int o = 0; o < got.Length; o += 4) {
                            int al = topDown[o + 3];
                            for (int ch = 0; ch < 3; ch++) {
                                int want = (topDown[o + ch] * al + 255 * (255 - al) + 127) / 255;
                                worst = Math.Max(worst, Math.Abs(want - got[o + ch]));
                            }
                        }
                    }
                }
                bool ok = a != IntPtr.Zero && b != IntPtr.Zero && ha[0] == fhx && ha[1] == fhy && hb[0] == fhx && hb[1] == fhy && worst >= 0 && worst <= 2;
                if (!ok) problems++;
                report.Add(string.Format("{0,3} {1,-28} {2,3} px  file hot {3},{4}  LoadImage hot {5},{6}  CreateIconIndirect hot {7},{8}  max diff {9}  {10}",
                    i, name, side, fhx, fhy, ha[0], ha[1], hb[0], hb[1], worst, ok ? "ok" : "PROBLEM"));
                if (a != IntPtr.Zero) DestroyCursor(a);
                if (b != IntPtr.Zero) DestroyCursor(b);
            }
            Png(Path.Combine(outDir, "cursors-sheet.png"), sheet.Pixels(), sheet.W, sheet.H);
        }
        File.WriteAllLines(Path.Combine(outDir, "cursors.txt"), report);
        foreach (var line in report) Console.WriteLine(line);
        return problems;
    }
}
'@
$files = @(Get-ChildItem -Path $CurDir -Filter *.cur -ErrorAction SilentlyContinue)
if ($files.Count -eq 0) { '::warning::no cursor files in ' + $CurDir + ' (a build from before NOSHAPE_CURSOR_DUMP)'; exit 0 }
$problems = [CursorCheck]::Run($CurDir, $OutDir)
if ($problems -gt 0) { "::error::$problems cursor(s) failed the Windows check (see cursors.txt)"; exit 1 }
"$($files.Count) cursors checked"
