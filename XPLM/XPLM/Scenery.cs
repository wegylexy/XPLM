using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;
public enum ProbeType
{
    Y
}

public enum ProbeResult
{
    HitTerrain,
    Error,
    Missed
}

[StructLayout(LayoutKind.Sequential)]
public readonly struct ProbeInfo
{
    readonly int StructSize;
    public readonly float LocationX, LocationY, LocationZ,
        NormalX, NormalY, NormalZ,
        VelocityX, VelocityY, VelocityZ;
    readonly int _IsWet;

    public bool IsWet => _IsWet != 0;
}

public sealed partial class Probe : IDisposable
{
    readonly nint _id;

    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMCreateProbe(ProbeType probeType);

    public Probe(ProbeType probeType)
    {
        _id = XPLMCreateProbe(probeType);
    }

    ~Probe() => Dispose();

    bool _disposed;
    public void Dispose()
    {
        [LibraryImport(Defs.Lib)]
        static extern void XPLMDestroyProbe(nint id);

        if (!_disposed)
        {
            XPLMDestroyProbe(_id);
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }

    [LibraryImport(Defs.Lib)]
    private static partial ProbeResult XPLMProbeTerrainXYZ(nint id, float x, float y, float z, ref ProbeInfo info);

    public unsafe ProbeResult TerrainXYZ(float x, float y, float z, out ProbeInfo info)
    {
        fixed (void* p = &info)
        {
            *(int*)p = sizeof(ProbeInfo);
        }
        return XPLMProbeTerrainXYZ(_id, x, y, z, ref info);
    }
}

public sealed partial class SceneryObject : IDisposable
{
    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial void XPLMLoadObjectAsync(string path, delegate* unmanaged<nint, nint, void> callback, nint state);

    public static async Task<SceneryObject> LoadAsync(string path)
    {
        TaskCompletionSource<SceneryObject> tcs = new();
        var h = GCHandle.Alloc(tcs);
        try
        {
            unsafe
            {
                [UnmanagedCallersOnly]
                static void L(nint id, nint state)
                {
                    var tcs = (TaskCompletionSource<SceneryObject>)GCHandle.FromIntPtr(state).Target!;
                    if (id == 0)
                    {
                        tcs.SetException(new InvalidOperationException());
                    }
                    else
                    {
                        tcs.SetResult(new(id));
                    }
                }

                XPLMLoadObjectAsync(path, &L, GCHandle.ToIntPtr(h));
            }
            await tcs.Task.ConfigureAwait(false);
        }
        finally
        {
            h.Free();
        }
        return tcs.Task.Result;
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial int XPLMLookupObjects(string path, float latitude, float longitude, delegate* unmanaged<nint, nint, void> enumerator, nint state);

    public static unsafe IEnumerable<string> Lookup(string path, float latitude, float longitude)
    {
        [UnmanagedCallersOnly]
        static void E(nint filePath, nint state) =>
            ((List<string>)GCHandle.FromIntPtr(state).Target!).Add(Marshal.PtrToStringUTF8(filePath)!);

        List<string> s = [];
        var h = GCHandle.Alloc(s);
        _ = XPLMLookupObjects(path, latitude, longitude, &E, GCHandle.ToIntPtr(h));
        h.Free();
        return s;
    }

    internal readonly nint _id;

    SceneryObject(nint id) => _id = id;

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial nint XPLMLoadObject(string path);

    public SceneryObject(string path)
    {
        _id = XPLMLoadObject(path);
        if (_id == 0)
        {
            throw new InvalidOperationException();
        }
    }

    ~SceneryObject() => Dispose();

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMUnloadObject(nint id);

    bool _disposed;
    public void Dispose()
    {
        if (!_disposed)
        {
            XPLMUnloadObject(_id);
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }
}

public static partial class Scenery
{
#pragma warning disable CA1401 // P/Invokes should not be visible
    [LibraryImport(Defs.Lib, EntryPoint = "XPLMGetMagneticVariation")]
    public static partial float GetMagneticVariation(double latitude, double longitude);

    [LibraryImport(Defs.Lib, EntryPoint = "XPLMDegTrueToDegMagnetic")]
    public static partial float DegTrueToDegMagnetic(float headingDegreesTrue);

    [LibraryImport(Defs.Lib, EntryPoint = "XPLMDegMagneticToDegTrue")]
    public static partial float DegMagneticToDegTrue(float headingDegreesMagnetic);
#pragma warning restore CA1401 // P/Invokes should not be visible
}