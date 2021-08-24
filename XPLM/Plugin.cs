using System.Collections;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;
public static class PluginMessages
{
    public const int
        PlaneCrashed = 101,
        PlaneLoaded = 102,
        AirportLoaded = 103,
        SceneryLoaded = 104,
        AirplaneCountChanged = 105,
        PlaneUnloaded = 106,
        WillWritePrefs = 107,
        LiveryLoaded = 108,
        EnteredVR = 109,
        ExitingVR = 110,
        ReleasePlanes = 111;
}

public sealed record PluginInfo
{
    public int ID { get; }
    public string Name { get; }
    public string FilePath { get; }
    public string Signature { get; }
    public string Description { get; }

    internal PluginInfo(int id, string name, string filePath, string signature, string description) =>
        (ID, Name, FilePath, Signature, Description) = (id, name, filePath, signature, description);

    public bool IsEnabled
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMIsPluginEnabled(int id);

            return XPLMIsPluginEnabled(ID) != 0;
        }
        set
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMEnablePlugin(int id);

            [DllImport(Defs.Lib)]
            static extern void XPLMDisablePlugin(int id);

            if (value)
                _ = XPLMEnablePlugin(ID);
            else
                XPLMDisablePlugin(ID);
        }
    }
}

sealed class PluginList : IReadOnlyList<PluginInfo>
{
    public PluginInfo this[int index]
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMGetNthPlugin(int index);

            return Plugin.GetInfo(XPLMGetNthPlugin(index))!;
        }
    }

    public int Count
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMCountPlugins();

            return XPLMCountPlugins();
        }
    }

    public IEnumerator<PluginInfo> GetEnumerator()
    {
        for (int i = 0, c = Count; i < c; ++i)
            yield return this[i];
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

public sealed record PluginFeature
{
    public string Feature { get; }

    internal PluginFeature(string feature) => Feature = feature;

    public bool IsEnabled
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMIsFeatureEnabled([MarshalAs(UnmanagedType.LPUTF8Str)] string feature);

            return XPLMIsFeatureEnabled(Feature) != 0;
        }
        set
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMEnableFeature([MarshalAs(UnmanagedType.LPUTF8Str)] string feature, int enable);

            XPLMEnableFeature(Feature, value ? 1 : 0);
        }
    }
}

public static class Plugin
{
    public static int MyID
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMGetMyID();

            return XPLMGetMyID();
        }
    }

    internal static PluginInfo? GetInfo(int id)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMGetPluginInfo(int id, in byte name, in byte filePath, in byte signature, in byte description);

        if (id == -1)
            return null;
        ReadOnlySpan<byte> name = stackalloc byte[256],
            filePath = stackalloc byte[256],
            signature = stackalloc byte[256],
            description = stackalloc byte[256];
        XPLMGetPluginInfo(id,
            in MemoryMarshal.AsRef<byte>(name),
            in MemoryMarshal.AsRef<byte>(filePath),
            in MemoryMarshal.AsRef<byte>(signature),
            in MemoryMarshal.AsRef<byte>(description)
        );
        return new(id, name.GetString(), filePath.GetString(), signature.GetString(), description.GetString());
    }

    public static IReadOnlyList<PluginInfo> List { get; } = new PluginList();

    public static PluginInfo? FindByPath(string path)
    {
        [DllImport(Defs.Lib)]
        static extern int XPLMFindPluginByPath([MarshalAs(UnmanagedType.LPUTF8Str)] string path);

        return GetInfo(XPLMFindPluginByPath(path));
    }

    public static PluginInfo? FindBySignature(string signature)
    {
        [DllImport(Defs.Lib)]
        static extern int XPLMFindPluginBySignature([MarshalAs(UnmanagedType.LPUTF8Str)] string signature);

        return GetInfo(XPLMFindPluginBySignature(signature));
    }

    public static void ReloadAll()
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMReloadPlugins();

        XPLMReloadPlugins();
    }

    public static void BroadcastMessage(int message, nint param = 0) =>
        SendMessageTo(null, message, param);

    public static void SendMessageTo(PluginInfo? plugin, int message, nint param = 0)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMSendMessageToPlugin(int plugin, int message, nint param);

        XPLMSendMessageToPlugin(plugin?.ID ?? -1, message, param);
    }

    public static bool TryGetFeature(string featurelity, out PluginFeature feature)
    {
        [DllImport(Defs.Lib)]
        static extern int XPLMHasFeature([MarshalAs(UnmanagedType.LPUTF8Str)] string feature);

        if (XPLMHasFeature(featurelity) == 0)
            return false;
        feature = new(featurelity);
        return true;
    }

    public static unsafe IEnumerable<PluginFeature> EnumerateFeatures()
    {
        [DllImport(Defs.Lib)]
        static extern unsafe void XPLMEnumerateFeatures(delegate* unmanaged<nint, nint, void> enumerator, nint state);

        [UnmanagedCallersOnly]
        static void Enumerate(nint feature, nint state) =>
            ((List<PluginFeature>)GCHandle.FromIntPtr(state).Target!).Add(new(Marshal.PtrToStringUTF8(feature)!));

        List<PluginFeature> features = new();
        var h = GCHandle.Alloc(features);
        try
        {
            XPLMEnumerateFeatures(&Enumerate, GCHandle.ToIntPtr(h));
            return features;
        }
        finally
        {
            h.Free();
        }
    }
}