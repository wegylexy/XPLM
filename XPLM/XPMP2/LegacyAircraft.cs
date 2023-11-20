using System.Diagnostics;
using System.Numerics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace FlyByWireless.XPMP2;

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct Position
{
    private readonly long size = sizeof(Position);
    public double lat, lon, elevation;
    public float pitch, roll, heading;
    public fixed byte label[32];
    public float offsetScale = 1f;
    public bool clampToGround;
    public int aiPrio = 1;
    public fixed float label_color[4];
    public int multiIdx;

    public Position() => label_color[3] = label_color[2] = label_color[0] = 1;
}

[Flags]
public enum LightStatus
{
    None = 0,
    Taxi = 1 << 16,
    Landing = 1 << 17,
    Beacon = 1 << 18,
    Strobe = 1 << 19,
    Nav = 1 << 20
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct Surfaces
{
    private readonly long size = sizeof(Surfaces);
    public float
        gearPosition,
        flapRatio,
        spoilerRatio,
        speedBrakeRatio,
        slatRatio,
        wingSweep,
        thrust,
        yokePitch,
        yokeHeading,
        yokeRoll;
    public LightStatus lights;
    public float
        tireDeflect,
        tireRotDegree,
        tireRotRpm,
        engRotDegree,
        engRotRpm,
        propRotDegree,
        propRotRpm,
        reversRatio;
    public bool touchDown;

    public Surfaces() { }
}

public enum TransponderMode
{
    Standby,
    Mode3A,
    ModeC,
    ModeCLow,
    MOdeCIdent
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct Radar
{
    private readonly long size = sizeof(Radar);
    public long code;
    public TransponderMode mode = TransponderMode.ModeC;

    public Radar() { }
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct InfoTexts
{
    private readonly long size = sizeof(InfoTexts);
    public fixed byte
        tailNum[10],
        icaoAcType[5],
        manufacturer[40],
        model[40],
        icaoAirline[4],
        airline[40],
        flightNum[10],
        aptFrom[5],
        aptTo[5];

    public InfoTexts() { }
}

public partial class LegacyAircraft : IDisposable
{
    private enum PlaneCallbackResult
    {
        Unavailable,
        Unchanged,
        NewData
    }

    private enum PlaneDataType : long
    {
        Position = 1L << 1,
        Surfaces = 1L << 2,
        Radar = 1L << 3,
        InfoTexts = 1L << 4
    }

    private PlaneCallbackResult _position, _surfaces, _radar, _infoTexts;
    private Position position;
    private Surfaces surfaces;
    private Radar radar;
    private InfoTexts infoTexts;

    [UnmanagedCallersOnly]
    static unsafe PlaneCallbackResult PlaneCallback(uint plane, PlaneDataType dataType, void* data, nint refcon)
    {
        static PlaneCallbackResult Exchange(ref PlaneCallbackResult location, PlaneCallbackResult value) =>
            (PlaneCallbackResult)Interlocked.Exchange(ref Unsafe.As<PlaneCallbackResult, int>(ref location), (int)value);

        var aircraft = (LegacyAircraft)GCHandle.FromIntPtr(refcon).Target!;
        switch (dataType)
        {
            case PlaneDataType.Position when aircraft._position is PlaneCallbackResult.NewData:
                Unsafe.Copy(data, ref aircraft.position);
                return Exchange(ref aircraft._position, PlaneCallbackResult.Unchanged);
            case PlaneDataType.Surfaces when aircraft._surfaces is PlaneCallbackResult.NewData:
                Unsafe.Copy(data, ref aircraft.surfaces);
                return Exchange(ref aircraft._surfaces, PlaneCallbackResult.Unchanged);
            case PlaneDataType.Radar when aircraft._radar is PlaneCallbackResult.NewData:
                Unsafe.Copy(data, ref aircraft.radar);
                return Exchange(ref aircraft._radar, PlaneCallbackResult.Unchanged);
            case PlaneDataType.InfoTexts when aircraft._infoTexts is PlaneCallbackResult.NewData:
                Unsafe.Copy(data, ref aircraft.infoTexts);
                return Exchange(ref aircraft._infoTexts, PlaneCallbackResult.Unchanged);
        }
        return PlaneCallbackResult.Unavailable;
    }

    public uint ModeSId { get; }

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial uint XPMPCreatePlaneWithModelName(string? modelName, string icaoCode, string airline, string livery, delegate* unmanaged<uint, PlaneDataType, void*, nint, PlaneCallbackResult> dataFunc, nint refcon, uint modeSId = default);

    internal unsafe LegacyAircraft(string icaoType, string icaoAirline, string livery, uint modeSId = default, string? cslId = null) =>
        ModeSId = XPMPCreatePlaneWithModelName(cslId, icaoType, icaoAirline, livery, &PlaneCallback, (nint)GCHandle.Alloc(this, GCHandleType.WeakTrackResurrection), modeSId);

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPDestroyPlane(uint id);

    private bool _disposed;

    protected virtual void Dispose(bool disposing)
    {
        if (!_disposed)
        {
            XPMPDestroyPlane(ModeSId);
            _disposed = true;
        }
    }

    ~LegacyAircraft() => Dispose(false);

    public void Dispose()
    {
        Dispose(disposing: true);
        GC.SuppressFinalize(this);
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPSetPlaneVisibility(uint id, [MarshalAs(UnmanagedType.I1)] bool visible);

    public bool Visible
    {
        set => XPMPSetPlaneVisibility(ModeSId, value);
    }

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPMPChangePlaneModel(uint planeId, string? icaoCode, string? airline, string? livery);

    public int ChangeModel(string? icaoType, string? icaoAirline, string? livery) =>
        XPMPChangePlaneModel(ModeSId, icaoType, icaoAirline, livery);

    [LibraryImport(nameof(XPMP2))]
    private static partial int XPMPGetPlaneModelName(uint id, nint txtBuf, int txtBufSize);

    public string ModelName
    {
        get
        {
            var length = XPMPGetPlaneModelName(ModeSId, default, 0);
            Debug.Assert(length >= 0);
            if (length <= 0)
            {
                return string.Empty;
            }
            Span<byte> s = stackalloc byte[length + 1];
            unsafe
            {
                length = XPMPGetPlaneModelName(ModeSId, (nint)Unsafe.AsPointer(ref s.GetPinnableReference()), s.Length);
                Debug.Assert(length == s.Length - 1);
            }
            return Encoding.UTF8.GetString(s[..^1]);
        }
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial int XPMPGetPlaneModelQuality(uint plane);

    public int ModelQuality => XPMPGetPlaneModelQuality(ModeSId);

    public void SetLocation(double lat, double lon, double alt_ft)
    {
        position.lat = lat;
        position.lon = lon;
        position.elevation = alt_ft;
        _position = PlaneCallbackResult.NewData;
    }

    public void GetLocation(out double lat, out double lon, out double alt_ft)
    {
        lat = position.lat;
        lon = position.lon;
        alt_ft = position.elevation;
    }

    public Vector3 Orientation
    {
        get => new(position.pitch, position.heading, position.roll);
        set
        {
            position.pitch = value.X;
            position.heading = value.Y;
            position.roll = value.Z;
            _position = PlaneCallbackResult.NewData;
        }
    }
    
    // TODO: implement
}
