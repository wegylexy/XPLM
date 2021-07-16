using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM
{
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

    public sealed class Command : IDisposable
    {
        public static Command? Find(string name)
        {
            [DllImport(Defs.Lib)]
            static extern nint XPLMFindCommand([MarshalAs(UnmanagedType.LPUTF8Str)] string name);

            var i = XPLMFindCommand(name);
            return i == 0 ? null : new(i);
        }

        public static Command Create(string name, string description)
        {
            [DllImport(Defs.Lib)]
            static extern nint XPLMCreateCommand([MarshalAs(UnmanagedType.LPUTF8Str)] string name, [MarshalAs(UnmanagedType.LPUTF8Str)] string description);

            return new(XPLMCreateCommand(name, description));
        }

        [DllImport(Defs.Lib)]
        static extern unsafe void XPLMRegisterCommandHandler(nint id, delegate* unmanaged<nint, CommandPhase, nint, int> handler, int before, nint state);

        [DllImport(Defs.Lib)]
        static extern unsafe void XPLMUnregisterCommandHandler(nint id, delegate* unmanaged<nint, CommandPhase, nint, int> handler, int before, nint state);

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
                Utilities.DebugString(ex.ToString());
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
                Utilities.DebugString(ex.ToString());
                return 1;
            }
        }

        internal readonly nint _id;

        readonly GCHandle _handle;

        event CommandCallback? _Before, _After;

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

        public void Begin()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMCommandBegin(nint id);

            XPLMCommandBegin(_id);
        }

        public void End()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMCommandEnd(nint id);

            XPLMCommandEnd(_id);
        }

        public void Once()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMCommandOnce(nint id);

            XPLMCommandOnce(_id);
        }
    }

    public static class Utilities
    {
        public static string SystemPath
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMGetSystemPath(ref byte systemPath);

                Span<byte> s = stackalloc byte[512];
                XPLMGetSystemPath(ref MemoryMarshal.GetReference(s));
                return s.GetString();
            }
        }

        public static string PrefsPath
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMGetPrefsPath(ref byte systemPath);

                Span<byte> s = stackalloc byte[512];
                XPLMGetPrefsPath(ref MemoryMarshal.GetReference(s));
                return s.GetString();
            }
        }

        public static char DirectorySeparator
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern ref byte XPLMGetDirectorySeparator();

                return (char)XPLMGetDirectorySeparator();
            }
        }

        public static string ExtractFileAndPath(ref string fullPath)
        {
            var i = fullPath.LastIndexOf(DirectorySeparator);
            var file = fullPath[(i + 1)..];
            fullPath = fullPath[..i];
            return file;
        }

        public static IEnumerable<string> EnumerateDirectoryContents(string directoryPath) =>
            Directory.EnumerateFiles(directoryPath);

        public static bool LoadDataFile(DataFileType fileType, string? filePath)
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMLoadDataFile(DataFileType fileType, [MarshalAs(UnmanagedType.LPUTF8Str)] string? filePath);

            return XPLMLoadDataFile(fileType, filePath) != 0;
        }

        public static bool SaveDataFile(DataFileType fileType, string? filePath)
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMSaveDataFile(DataFileType fileType, [MarshalAs(UnmanagedType.LPUTF8Str)] string? filePath);

            return XPLMSaveDataFile(fileType, filePath) != 0;
        }

        public static (int XPlaneVersion, int XPLMVersion) Versions
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMGetVersions(out int xPlaneVersion, out int xplmVersion, out int hostID);

                XPLMGetVersions(out var xPlane, out var xplm, out _);
                return (xPlane, xplm);
            }
        }

        public static LanguageCode Language
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern LanguageCode XPLMGetLanguage();

                return XPLMGetLanguage();
            }
        }

        public static nint FindSymbol(string symbol)
        {
            [DllImport(Defs.Lib)]
            static extern nint XPLMFindSymbol([MarshalAs(UnmanagedType.LPUTF8Str)] string symbol);

            return XPLMFindSymbol(symbol);
        }

        static Action<string>? _ErrorCallback;
        public static unsafe Action<string>? ErrorCallback
        {
            set
            {
                [DllImport(Defs.Lib)]
                static extern unsafe void XPLMSetErrorCallback(delegate* unmanaged<nint, void> callback);

                [UnmanagedCallersOnly]
                static void E(nint message)
                {
                    try
                    {
                        _ErrorCallback!(Marshal.PtrToStringUTF8(message)!);
                    }
                    catch (Exception ex)
                    {
                        DebugString(ex.ToString());
                    }
                }

                _ErrorCallback = value;
                XPLMSetErrorCallback(value != null ? &E : null);
            }
        }

        public static void DebugString(string debug)
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMDebugString([MarshalAs(UnmanagedType.LPUTF8Str)] string debug);

            XPLMDebugString(debug);
        }

        public static void SpeakString(string speak)
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMSpeakString([MarshalAs(UnmanagedType.LPUTF8Str)] string debug);

            XPLMSpeakString(speak);
        }

        public static string GetVirtualKeyDescription(VirtualKey virtualKey)
        {
            [DllImport(Defs.Lib)]
            [return: MarshalAs(UnmanagedType.LPUTF8Str)]
            static extern string XPLMGetVirtualKeyDescription(VirtualKey virtualKey);

            return XPLMGetVirtualKeyDescription(virtualKey);
        }

        public static void ReloadScenery()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMReloadScenery();

            XPLMReloadScenery();
        }
    }
}