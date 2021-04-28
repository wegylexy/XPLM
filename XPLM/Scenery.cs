using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace FlyByWireless.XPLM
{
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

    public sealed class Probe : IDisposable
    {
        readonly nint _id;

        public Probe(ProbeType probeType)
        {
            [DllImport(Defs.Lib)]
            static extern nint XPLMCreateProbe(ProbeType probeType);

            _id = XPLMCreateProbe(probeType);
        }

        ~Probe() => Dispose();

        bool _disposed;
        public void Dispose()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMDestroyProbe(nint id);

            if (!_disposed)
            {
                XPLMDestroyProbe(_id);
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        public unsafe ProbeResult TerrainXYZ(float x, float y, float z, out ProbeInfo info)
        {
            [DllImport(Defs.Lib)]
            static extern ProbeResult XPLMProbeTerrainXYZ(nint id, float x, float y, float z, ref ProbeInfo info);

            fixed (void* p = &info)
                *(int*)p = sizeof(ProbeInfo);
            return XPLMProbeTerrainXYZ(_id, x, y, z, ref info);
        }
    }

    public sealed class SceneryObject : IDisposable
    {
        public static async Task<SceneryObject> LoadAsync(string path)
        {
            TaskCompletionSource<SceneryObject> tcs = new();
            var h = GCHandle.Alloc(tcs);
            unsafe
            {
                [DllImport(Defs.Lib)]
                static extern unsafe void XPLMLoadObjectAsync([MarshalAs(UnmanagedType.LPUTF8Str)] string path, delegate* unmanaged<nint, nint, void> callback, nint state);

                [UnmanagedCallersOnly]
                static void L(nint id, nint state)
                {
                    var tcs = (TaskCompletionSource<SceneryObject>)GCHandle.FromIntPtr(state).Target!;
                    if (id == 0)
                        tcs.SetException(new InvalidOperationException());
                    else
                        tcs.SetResult(new(id));
                }

                XPLMLoadObjectAsync(path, &L, GCHandle.ToIntPtr(h));
            }
            var o = await tcs.Task.ConfigureAwait(false);
            h.Free();
            return o;
        }

        public static unsafe IEnumerable<string> Lookup(string path, float latitude, float longitude)
        {
            [DllImport(Defs.Lib)]
            static extern unsafe int XPLMLookupObjects([MarshalAs(UnmanagedType.LPUTF8Str)] string path, float latitude, float longitude, delegate* unmanaged<nint, nint, void> enumerator, nint state);

            [UnmanagedCallersOnly]
            static void E(nint filePath, nint state) =>
                ((List<string>)GCHandle.FromIntPtr(state).Target!).Add(Marshal.PtrToStringUTF8(filePath)!);

            List<string> s = new();
            var h = GCHandle.Alloc(s);
            _ = XPLMLookupObjects(path, latitude, longitude, &E, GCHandle.ToIntPtr(h));
            h.Free();
            return s;
        }

        internal readonly nint _id;

        SceneryObject(nint id) => _id = id;

        public SceneryObject(string path)
        {
            [DllImport(Defs.Lib)]
            static extern nint XPLMLoadObject([MarshalAs(UnmanagedType.LPUTF8Str)] string path);

            _id = XPLMLoadObject(path);
            if (_id == 0)
                throw new InvalidOperationException();
        }

        ~SceneryObject() => Dispose();

        bool _disposed;
        public void Dispose()
        {
            if (!_disposed)
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMUnloadObject(nint id);

                XPLMUnloadObject(_id);
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }
    }

    public static class Scenery
    {
        public static float GetMagneticVariation(double latitude, double longitude)
        {
            [DllImport(Defs.Lib)]
            static extern float XPLMGetMagneticVariation(double latitude, double longitude);

            return XPLMGetMagneticVariation(latitude, longitude);
        }

        public static float DegTrueToDegMagnetic(float headingDegreesTrue)
        {
            [DllImport(Defs.Lib)]
            static extern float XPLMDegTrueToDegMagnetic(float headingDegreesTrue);

            return XPLMDegTrueToDegMagnetic(headingDegreesTrue);
        }

        public static float DegMagneticToDegTrue(float headingDegreesMagnetic)
        {
            [DllImport(Defs.Lib)]
            static extern float XPLMDegMagneticToDegTrue(float headingDegreesMagnetic);

            return XPLMDegMagneticToDegTrue(headingDegreesMagnetic);
        }
    }
}