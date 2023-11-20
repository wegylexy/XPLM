using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public enum DataFileType
{
    Invalid,
    Situation,
    ReplayMovie
}

public enum LanguageCode
{
    Unknown,
    English,
    French,
    German,
    Italian,
    Spanish,
    Korean,
    Russian,
    Greek,
    Japanese,
    Chinese
}

public enum CommandPhase
{
    Begin,
    Continue,
    End
}

public delegate bool CommandCallback(Command command, CommandPhase phase);

public sealed partial class Command : IDisposable
{
    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMFindCommand(ReadOnlySpan<byte> name);

    public static Command? Find(ReadOnlySpan<byte> name)
    {
        var i = XPLMFindCommand(name);
        return i == 0 ? null : new(i);
    }

    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMCreateCommand(ReadOnlySpan<byte> name, ReadOnlySpan<byte> description);

    public static Command Create(ReadOnlySpan<byte> name, ReadOnlySpan<byte> description) =>
        new(XPLMCreateCommand(name, description));

    [LibraryImport(Defs.Lib)]
    private static unsafe partial void XPLMRegisterCommandHandler(nint id, delegate* unmanaged<nint, CommandPhase, nint, int> handler, int before, nint state);

    [LibraryImport(Defs.Lib)]
    private static unsafe partial void XPLMUnregisterCommandHandler(nint id, delegate* unmanaged<nint, CommandPhase, nint, int> handler, int before, nint state);

    [UnmanagedCallersOnly]
    static int B(nint id, CommandPhase phase, nint state)
    {
        try
        {
            var c = (Command)GCHandle.FromIntPtr(state).Target!;
            return c._Before?.Invoke(c, phase) != false ? 1 : 0;
        }
        catch (Exception ex)
        {
            Utilities.DebugString(ex.ToString() + "\n");
            return 1;
        }
    }

    [UnmanagedCallersOnly]
    static int A(nint id, CommandPhase phase, nint state)
    {
        try
        {
            var c = (Command)GCHandle.FromIntPtr(state).Target!;
            return c._After?.Invoke(c, phase) != false ? 1 : 0;
        }
        catch (Exception ex)
        {
            Utilities.DebugString(ex.ToString() + "\n");
            return 1;
        }
    }

    internal readonly nint _id;

    readonly GCHandle _handle;

    private event CommandCallback? _Before, _After;

    public unsafe event CommandCallback Before
    {
        add
        {
            if (_Before is null)
                XPLMRegisterCommandHandler(_id, &B, 1, GCHandle.ToIntPtr(_handle));
            _Before += value;
        }
        remove
        {
            _Before -= value;
            if (_Before is null)
                XPLMUnregisterCommandHandler(_id, &B, 1, GCHandle.ToIntPtr(_handle));
        }
    }

    public unsafe event CommandCallback After
    {
        add
        {
            if (_After is null)
                XPLMRegisterCommandHandler(_id, &A, 1, GCHandle.ToIntPtr(_handle));
            _After += value;
        }
        remove
        {
            _After -= value;
            if (_After is null)
                XPLMUnregisterCommandHandler(_id, &A, 1, GCHandle.ToIntPtr(_handle));
        }
    }

    internal Command(nint id) => (_id, _handle) = (id, GCHandle.Alloc(this));

    bool _disposed;
    public void Dispose()
    {
        if (!_disposed)
        {
            _handle.Free();
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMCommandBegin(nint id);

    public void Begin() => XPLMCommandBegin(_id);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMCommandEnd(nint id);

    public void End() => XPLMCommandEnd(_id);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMCommandOnce(nint id);

    public void Once() => XPLMCommandOnce(_id);
}

public static partial class Utilities
{
    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGetSystemPath(in byte systemPath);

    public static string SystemPath
    {
        get
        {
            ReadOnlySpan<byte> s = stackalloc byte[512];
            XPLMGetSystemPath(in MemoryMarshal.GetReference(s));
            return s.GetString();
        }
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGetPrefsPath(in byte systemPath);

    public static string PrefsPath
    {
        get
        {
            ReadOnlySpan<byte> s = stackalloc byte[512];
            XPLMGetPrefsPath(in MemoryMarshal.GetReference(s));
            return s.GetString();
        }
    }

    [LibraryImport(Defs.Lib)]
    private static unsafe partial byte* XPLMGetDirectorySeparator();

    public static unsafe char DirectorySeparator =>
        (char)*XPLMGetDirectorySeparator();

    public static string ExtractFileAndPath(ref string fullPath)
    {
        var i = fullPath.LastIndexOf(DirectorySeparator);
        var file = fullPath[(i + 1)..];
        fullPath = fullPath[..i];
        return file;
    }

    public static IEnumerable<string> EnumerateDirectoryContents(string directoryPath) =>
        Directory.EnumerateFiles(directoryPath);

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPLMLoadDataFile(DataFileType fileType, string? filePath);

    public static bool LoadDataFile(DataFileType fileType, string? filePath) =>
        XPLMLoadDataFile(fileType, filePath) != 0;

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPLMSaveDataFile(DataFileType fileType, string? filePath);

    public static bool SaveDataFile(DataFileType fileType, string? filePath) =>
        XPLMSaveDataFile(fileType, filePath) != 0;

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGetVersions(out int xPlaneVersion, out int xplmVersion, out int hostID);

    public static (int XPlaneVersion, int XPLMVersion) Versions
    {
        get
        {
            XPLMGetVersions(out var xPlane, out var xplm, out _);
            return (xPlane, xplm);
        }
    }

#pragma warning disable CA1401 // P/Invokes should not be visible
    [LibraryImport(Defs.Lib, EntryPoint = "XPLMGetLanguage")]
    public static partial LanguageCode GetLanguage();
#pragma warning restore CA1401 // P/Invokes should not be visible

    [LibraryImport(Defs.Lib, EntryPoint = "XPLMFindSymbol")]
    public static partial nint FindSymbol(ReadOnlySpan<byte> symbol);

    [LibraryImport(Defs.Lib)]
    private static unsafe partial void XPLMSetErrorCallback(delegate* unmanaged<nint, void> callback);

    static Action<string>? _ErrorCallback;
    public static unsafe Action<string>? ErrorCallback
    {
        set
        {
            [UnmanagedCallersOnly]
            static void E(nint message)
            {
                try
                {
                    _ErrorCallback!(Marshal.PtrToStringUTF8(message)!);
                }
                catch (Exception ex)
                {
                    DebugString(ex.ToString() + "\n");
                }
            }

            _ErrorCallback = value;
            XPLMSetErrorCallback(value != null ? &E : null);
        }
    }

    [LibraryImport(Defs.Lib, EntryPoint = "XPLMDebugString", StringMarshalling = StringMarshalling.Utf8)]
    public static partial void DebugString(string debug);

    [LibraryImport(Defs.Lib, EntryPoint = "XPLMSpeakString", StringMarshalling = StringMarshalling.Utf8)]
    public static partial void SpeakString(string debug);

    [LibraryImport(Defs.Lib, EntryPoint = "XPLMGetVirtualKeyDescription", StringMarshalling = StringMarshalling.Utf8)]
    public static partial string GetVirtualKeyDescription(VirtualKey virtualKey);

#pragma warning disable CA1401 // P/Invokes should not be visible
    [LibraryImport(Defs.Lib, EntryPoint = "XPLMReloadScenery")]
    public static partial void ReloadScenery();
#pragma warning restore CA1401 // P/Invokes should not be visible
}