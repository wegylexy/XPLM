using System.Collections;
using System.Diagnostics.CodeAnalysis;
using System.Runtime.InteropServices;
using System.Text;

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

public sealed partial record PluginInfo
{
    public int ID { get; }
    public string Name { get; }
    public string FilePath { get; }
    public string Signature { get; }
    public string Description { get; }

    internal PluginInfo(int id, string name, string filePath, string signature, string description) =>
        (ID, Name, FilePath, Signature, Description) = (id, name, filePath, signature, description);

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMIsPluginEnabled(int id);

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMEnablePlugin(int id);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDisablePlugin(int id);

    public bool IsEnabled
    {
        get => XPLMIsPluginEnabled(ID) != 0;
        set
        {
            if (value)
                _ = XPLMEnablePlugin(ID);
            else
                XPLMDisablePlugin(ID);
        }
    }
}

sealed partial class PluginList : IReadOnlyList<PluginInfo>
{
    [LibraryImport(Defs.Lib)]
    private static partial int XPLMGetNthPlugin(int index);

    public PluginInfo this[int index] => Plugin.GetInfo(XPLMGetNthPlugin(index))!;

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMCountPlugins();

    public int Count => XPLMCountPlugins();

    public IEnumerator<PluginInfo> GetEnumerator()
    {
        for (int i = 0, c = Count; i < c; ++i)
            yield return this[i];
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
}

public sealed partial record PluginFeature
{
    public string Feature { get; }

    internal PluginFeature(string feature) => Feature = feature;

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPLMIsFeatureEnabled(string feature);

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPLMEnableFeature(string feature, int enable);

    public bool IsEnabled
    {
        get => XPLMIsFeatureEnabled(Feature) != 0;
        set => XPLMEnableFeature(Feature, value ? 1 : 0);
    }
}

public static partial class Plugin
{
    [LibraryImport(Defs.Lib)]
    private static partial int XPLMGetMyID();

    public static int MyID => XPLMGetMyID();

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGetPluginInfo(int id, in byte name, in byte filePath, in byte signature, in byte description);

    internal static PluginInfo? GetInfo(int id)
    {
        if (id == -1)
        {
            return null;
        }
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

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPLMFindPluginByPath(string path);

    public static PluginInfo? FindByPath(string path) =>
        GetInfo(XPLMFindPluginByPath(path));

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMFindPluginBySignature(ReadOnlySpan<byte> signature);

    public static PluginInfo? FindBySignature(ReadOnlySpan<byte> signature) =>
        GetInfo(XPLMFindPluginBySignature(signature));

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMReloadPlugins();

    public static void ReloadAll() =>
        XPLMReloadPlugins();

    public static void BroadcastMessage(int message, nint param = 0) =>
        SendMessageTo(null, message, param);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSendMessageToPlugin(int plugin, int message, nint param);

    public static void SendMessageTo(PluginInfo? plugin, int message, nint param = 0) =>
        XPLMSendMessageToPlugin(plugin?.ID ?? -1, message, param);

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMHasFeature(ReadOnlySpan<byte> feature);

    public static bool TryGetFeature(ReadOnlySpan<byte> featurelity, [MaybeNullWhen(false)] out PluginFeature? feature)
    {
        if (XPLMHasFeature(featurelity) == 0)
        {
            feature = null;
            return false;
        }
        feature = new(Encoding.UTF8.GetString(featurelity));
        return true;
    }

    [LibraryImport(Defs.Lib)]
    private static unsafe partial void XPLMEnumerateFeatures(delegate* unmanaged<nint, nint, void> enumerator, nint state);

    public static unsafe IEnumerable<PluginFeature> EnumerateFeatures()
    {
        [UnmanagedCallersOnly]
        static void Enumerate(nint feature, nint state) =>
            ((List<PluginFeature>)GCHandle.FromIntPtr(state).Target!).Add(new(Marshal.PtrToStringUTF8(feature)!));

        List<PluginFeature> features = [];
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