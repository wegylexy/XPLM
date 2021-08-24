using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace FlyByWireless.XPLM;

[StructLayout(LayoutKind.Sequential)]
public readonly struct DrawInfo
{
    readonly int StructSize;
    public readonly float X, Y, Z, Pitch, Heading, Roll;

    public DrawInfo(float x, float y, float z, float pitch, float heading, float roll) =>
        (StructSize, X, Y, Z, Pitch, Heading, Roll) =
            (Unsafe.SizeOf<DrawInfo>(), x, y, z, pitch, heading, roll);
}

public sealed class Instance : IDisposable
{
    readonly nint _id;

    public Instance(SceneryObject obj, IEnumerable<string> datarefs)
    {
        [DllImport(Defs.Lib)]
        static extern nint XPLMCreateInstance(nint obj, ref nint datarefs);

        var hs = datarefs?.Select(d => GCHandle.Alloc(Encoding.UTF8.GetBytes(d), GCHandleType.Pinned)).ToList() ?? new();
        Span<nint> s = stackalloc nint[hs.Count + 1];
        for (var i = 0; i < hs.Count; ++i)
        {
            s[i] = hs[i].AddrOfPinnedObject();
        }
        s[hs.Count] = 0;
        _id = XPLMCreateInstance(obj._id, ref MemoryMarshal.GetReference(s));
        foreach (var h in hs)
        {
            h.Free();
        }
    }

    bool _disposed;
    public void Dispose()
    {
        if (!_disposed)
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMDestroyInstance(nint id);

            XPLMDestroyInstance(_id);
            _disposed = true;
        }
    }

    public void SetPosition(in DrawInfo newPosition, ReadOnlySpan<float> data)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMInstanceSetPosition(nint id, in DrawInfo newPosition, ref float data);

        XPLMInstanceSetPosition(_id, in newPosition, ref MemoryMarshal.GetReference(data));
    }
}