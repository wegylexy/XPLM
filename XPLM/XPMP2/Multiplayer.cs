using FlyByWireless.XPLM;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPMP2;

public abstract partial class XPluginMultiplayer : XPluginBase
{
    public static Dictionary<string, int> IntPrefs { get; } = [];

    private static Dictionary<nint, string> _IntPrefs = [];

    public abstract string ResourceDir { get; }

    public virtual ReadOnlySpan<byte> DefaultICAO => null;

    public virtual ReadOnlySpan<byte> LogAcronym => null;

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial string XPMPMultiplayerInit(ReadOnlySpan<byte> pluginName, string resourceDir, delegate* unmanaged<nint, nint, int, int> intPrefsFunc, ReadOnlySpan<byte> defaultICAO, ReadOnlySpan<byte> pluginLogAcronym);

    public unsafe XPluginMultiplayer()
    {
        [UnmanagedCallersOnly]
        static int IntPrefsFunc(nint section, nint key, int @default)
        {
            if (!_IntPrefs.TryGetValue(key, out var s))
            {
                s = Marshal.PtrToStringUTF8(key)!;
                _IntPrefs.Add(key, s);
            }
            return IntPrefs.TryGetValue(s, out var v) ? v : @default;
        }

        var e = XPMPMultiplayerInit(Name, ResourceDir, &IntPrefsFunc, DefaultICAO, LogAcronym);
        if (e is { Length: > 0 })
        {
            throw new ApplicationException(e);
        }
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPMultiplayerCleanup();

    private bool _disposed;
    protected override void Dispose(bool isDisposing)
    {
        if (!_disposed)
        {
            XPMPMultiplayerCleanup();
            _disposed = true;
        }
    }

    [LibraryImport(nameof(XPMP2))]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPContrailsAutoEnabled();

    public bool ContrailsAutoEnabled => XPMPContrailsAutoEnabled();

    [LibraryImport(nameof(XPMP2))]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPContrailsAvailable();

    public bool ContrailsAvailable => XPMPContrailsAvailable();

    [LibraryImport(nameof(XPMP2))]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPSoundEnable([MarshalAs(UnmanagedType.I1)] bool enable = true);

    [LibraryImport(nameof(XPMP2))]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPSoundIsEnabled();

    public bool IsSoundEnabled
    {
        get => XPMPSoundIsEnabled();
        set => XPMPSoundEnable(value);
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPSoundSetMasterVolume(float vol = 1f);

    public float MasterSoundVolume
    {
        set => XPMPSoundSetMasterVolume(value);
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPSoundMute([MarshalAs(UnmanagedType.I1)] bool mute);

    public bool MuteSound
    {
        set => XPMPSoundMute(value);
    }

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static partial string XPMPSoundAdd(string name, string filePath, [MarshalAs(UnmanagedType.I1)] bool loop,
        float coneDir = float.NaN, float conePitch = float.NaN, float coneInAngle = float.NaN, float coneOutAngle = float.NaN, float coneOutVol = float.NaN);

#pragma warning disable CA1822 // Mark members as static
    public void AddSound(string name, string filePath, bool loop,
        float coneDir = float.NaN, float conePitch = float.NaN, float coneInAngle = float.NaN, float coneOutAngle = float.NaN, float coneOutVol = float.NaN)
    {
        var e = XPMPSoundAdd(name, filePath, loop, coneDir, conePitch, coneInAngle, coneOutAngle, coneOutVol);
        if (e is { Length: > 0 })
        {
            throw new ApplicationException(e);
        }
    }

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static partial string? XPMPSoundEnumerate(string? prevName, out string filePath);

    public IEnumerable<KeyValuePair<string, string>> EnumerateSounds()
    {
        string? name = null;
        for (; ; )
        {
            name = XPMPSoundEnumerate(name, out var filePath);
            if (name is null)
            {
                break;
            }
            yield return new(name, filePath);
        }
    }

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial string? XPMPMultiplayerEnable(delegate* unmanaged<nint, void> callback = null, nint refCon = default);

    public unsafe void EnableMultiplayer()
    {
        var e = XPMPMultiplayerEnable();
        if (e is { Length: > 0 })
        {
            throw new ApplicationException(e);
        }
    }

    public async Task EnableMultiplayerAsync()
    {
        [UnmanagedCallersOnly]
        static void Callback(nint state)
        {
            var handle = GCHandle.FromIntPtr(state);
            var tcs = (TaskCompletionSource)handle.Target!;
            tcs.SetResult();
        }

        TaskCompletionSource tcs = new();
        var handle = GCHandle.Alloc(tcs, GCHandleType.Normal);
        try
        {
            string? e = null;
            unsafe
            {
                e = XPMPMultiplayerEnable(&Callback, (nint)handle);
                if (e is not { Length: > 0 })
                {
                    return;
                }
            }
            await tcs.Task;
            throw new ApplicationException(e);
        }
        finally
        {
            handle.Free();
        }
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPMultiplayerDisable();

    public void DisableMultiplayer() => XPMPMultiplayerDisable();

    [LibraryImport(nameof(XPMP2))]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPHasControlOfAIAircraft();

    public bool HasControlOfAIAircraft => XPMPHasControlOfAIAircraft();

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static partial string XPMPLoadCSLPackage(string cslFolder);

    public void LoadCSLPackage(string folder)
    {
        var e = XPMPLoadCSLPackage(folder);
        if (e is { Length: > 0 })
        {
            throw new ApplicationException(e);
        }
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial int XPMPGetNumberOfInstalledModels();

    public int NumberOfInstalledModels => XPMPGetNumberOfInstalledModels();

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPMPGetModelInfo(int index, out string modelName, out string icao, out string airline, out string livery);

    public void GetModelInfo(int index, out string modelName, out string icao, out string airline, out string livery) =>
        XPMPGetModelInfo(index, out modelName, out icao, out airline, out livery);

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPMPModelMatchQuality(string? icao, string? airline, string? livery);

    public int ModelMatchQuality(string? icao, string? airline, string? livery) =>
        XPMPModelMatchQuality(icao, airline, livery);

    [LibraryImport(nameof(XPMP2), StringMarshalling = StringMarshalling.Utf8)]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPIsICAOValid(string icao);

    public bool IsICAOValid(string icao) => XPMPIsICAOValid(icao);

    public LegacyAircraft CreateAircraft(string icaoType, string icaoAirline, string livery, uint modeSId = default, string? cslId = null) =>
        new(icaoType, icaoAirline, livery, modeSId, cslId);

    [LibraryImport(nameof(XPMP2))]
    private static partial long XPMPCountPlanes();

    public long AircraftCount => XPMPCountPlanes();

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPSetDefaultPlaneICAO(ReadOnlySpan<byte> acIcaoType, ReadOnlySpan<byte> carIcaoType = default);

    public void SetDefaultICAO(ReadOnlySpan<byte> aircraft, ReadOnlySpan<byte> vehicle = default) =>
        XPMPSetDefaultPlaneICAO(aircraft, vehicle);

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPEnableAircraftLabels([MarshalAs(UnmanagedType.I1)] bool enable = true);

    [LibraryImport(nameof(XPMP2))]
    [return: MarshalAs(UnmanagedType.I1)]
    private static partial bool XPMPDrawingAircraftLabels();

    public bool EnableAircraftLabels
    {
        set => XPMPEnableAircraftLabels(value);
        get => XPMPDrawingAircraftLabels();
    }

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPSetAircraftLabelDist(float dist_nm, [MarshalAs(UnmanagedType.I1)] bool cutOffAtVisibility = true);

    public void SetAircraftLabelDist(float dist_nm, bool cutOffAtVisibility = true) =>
        XPMPSetAircraftLabelDist(dist_nm, cutOffAtVisibility);

    [LibraryImport(nameof(XPMP2))]
    private static partial void XPMPEnableMap([MarshalAs(UnmanagedType.I1)] bool enable, [MarshalAs(UnmanagedType.I1)] bool labels = true);

    public void EnableMap(bool map, bool labels = true) =>
        XPMPEnableMap(map, labels);
#pragma warning restore CA1822 // Mark members as static
}
