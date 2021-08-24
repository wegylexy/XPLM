using System.Diagnostics;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public sealed class Planes : IDisposable
{
    public static void SetUsersAircraft(string aircraftPath)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMSetUsersAircraft([MarshalAs(UnmanagedType.LPUTF8Str)] string aircraftPath);

        XPLMSetUsersAircraft(aircraftPath);
    }

    public static void PlaceUserAtAirport(string airportCode)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMPlaceUserAtAirport([MarshalAs(UnmanagedType.LPUTF8Str)] string airportCode);

        XPLMPlaceUserAtAirport(airportCode);
    }

    public static void PlaceUserAtLocation(double latitudeDegrees, double longitudeDegrees, float elevationMetersMSL, float headingDegreesTrue, float speedMetersPerSecond)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMPlaceUserAtLocation(double latitudeDegrees, double longitudeDegrees, float elevationMetersMSL, float headingDegreesTrue, float speedMetersPerSecond);

        XPLMPlaceUserAtLocation(latitudeDegrees, longitudeDegrees, elevationMetersMSL, headingDegreesTrue, speedMetersPerSecond);
    }

    public static void CountAircraft(out int totalAircraft, out int activeAircraft, out int controller)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMCountAircraft(out int totalAircraft, out int activeAircraft, out int controller);

        XPLMCountAircraft(out totalAircraft, out activeAircraft, out controller);
    }

    public static void GetNthAircraftModel(int index, out string fileName, out string path)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMGetNthAircraftModel(int index, in byte fileName, in byte path);

        ReadOnlySpan<byte> n = stackalloc byte[256], p = stackalloc byte[512];
        XPLMGetNthAircraftModel(index, in MemoryMarshal.GetReference(n), in MemoryMarshal.GetReference(p));
        fileName = n.GetString();
        path = p.GetString();
    }

    static TaskCompletionSource<Planes>? _tcs; // TODO: use ManualResetValueTaskSourceCore<>

    public static unsafe ValueTask<Planes> AcquireAsync(IList<string> aircraft)
    {
        [UnmanagedCallersOnly]
        static void Available(nint refcon)
        {
            _tcs?.TrySetResult(new());
        }

        [DllImport(Defs.Lib)]
        static extern int XPLMAcquirePlanes(in nint aircraft, delegate* unmanaged<nint, void> callback, nint refcon);

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
            if (XPLMAcquirePlanes(in MemoryMarshal.GetReference(s), &Available, 0) != 0)
            {
                return ValueTask.FromResult<Planes>(new());
            }
            else
            {
                return new(_tcs.Task);
            }
        }
        finally
        {
            for (var i = 0; i < aircraft.Count; ++i)
            {
                Marshal.ZeroFreeCoTaskMemUTF8(s[i]);
            }
        }
    }

    bool _released;

    void Release()
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMReleasePlanes();

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

#pragma warning disable CA1822
    public void SetAircraftModel(int index, string aircraftPath)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMSetAircraftModel(int index, [MarshalAs(UnmanagedType.LPUTF8Str)] string aircraftPath);

        Debug.Assert(!_released);
        XPLMSetAircraftModel(index, aircraftPath);
    }

    public void DisableAIForPlane(int planeIndex)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMDisableAIForPlane(int planeIndex);

        Debug.Assert(!_released);
        XPLMDisableAIForPlane(planeIndex);
    }
#pragma warning restore CA1822
}