using System.Diagnostics;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public sealed partial class Planes : IDisposable
{
    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPLMSetUsersAircraft(string aircraftPath);

    public static void SetUsersAircraft(string aircraftPath)
    {
        XPLMSetUsersAircraft(aircraftPath);
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPLMPlaceUserAtAirport(string airportCode);

    public static void PlaceUserAtAirport(string airportCode)
    {
        XPLMPlaceUserAtAirport(airportCode);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMPlaceUserAtLocation(double latitudeDegrees, double longitudeDegrees, float elevationMetersMSL, float headingDegreesTrue, float speedMetersPerSecond);

    public static void PlaceUserAtLocation(double latitudeDegrees, double longitudeDegrees, float elevationMetersMSL, float headingDegreesTrue, float speedMetersPerSecond)
    {
        XPLMPlaceUserAtLocation(latitudeDegrees, longitudeDegrees, elevationMetersMSL, headingDegreesTrue, speedMetersPerSecond);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMCountAircraft(out int totalAircraft, out int activeAircraft, out int controller);

    public static void CountAircraft(out int totalAircraft, out int activeAircraft, out int controller)
    {
        XPLMCountAircraft(out totalAircraft, out activeAircraft, out controller);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGetNthAircraftModel(int index, in byte fileName, in byte path);

    public static void GetNthAircraftModel(int index, out string fileName, out string path)
    {
        ReadOnlySpan<byte> n = stackalloc byte[256], p = stackalloc byte[512];
        XPLMGetNthAircraftModel(index, in MemoryMarshal.GetReference(n), in MemoryMarshal.GetReference(p));
        fileName = n.GetString();
        path = p.GetString();
    }

    static TaskCompletionSource<Planes>? _tcs; // TODO: use ManualResetValueTaskSourceCore<>

    [LibraryImport(Defs.Lib)]
    private static unsafe partial int XPLMAcquirePlanes(in nint aircraft, delegate* unmanaged<nint, void> callback, nint refcon);

    public static unsafe ValueTask<Planes> AcquireAsync(IList<string> aircraft)
    {
        [UnmanagedCallersOnly]
        static void Available(nint refcon)
        {
            _tcs?.TrySetResult(new());
        }

        _tcs?.TrySetCanceled();
        // TODO: ensure singleton
        _tcs = new();
        Span<nint> s = stackalloc nint[aircraft.Count + 1];
        for (var i = 0; i < aircraft.Count; ++i)
        {
            s[i] = Marshal.StringToCoTaskMemUTF8(aircraft[i] ?? string.Empty);
        }
        try
        {
            return XPLMAcquirePlanes(in MemoryMarshal.GetReference(s), &Available, 0) != 0 ?
                ValueTask.FromResult<Planes>(new()) :
                new(_tcs.Task);
        }
        finally
        {
            for (var i = 0; i < aircraft.Count; ++i)
            {
                Marshal.ZeroFreeCoTaskMemUTF8(s[i]);
            }
        }
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMReleasePlanes();

    bool _released;

    void Release()
    {
        if (!_released)
        {
            XPLMReleasePlanes();
            _released = true;
        }
    }

    ~Planes() => Release();

    public void Dispose()
    {
        Release();
        GC.SuppressFinalize(this);
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPLMSetAircraftModel(int index, string aircraftPath);

    public void SetAircraftModel(int index, string aircraftPath)
    {
        Debug.Assert(!_released);
        XPLMSetAircraftModel(index, aircraftPath);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDisableAIForPlane(int planeIndex);

    public void DisableAIForPlane(int planeIndex)
    {
        Debug.Assert(!_released);
        XPLMDisableAIForPlane(planeIndex);
    }
}