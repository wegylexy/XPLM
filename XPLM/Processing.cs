using System;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM.Processing
{
    public enum FlightLoopPhase
    {
        BeforeFlightModel,
        AfterFlightModel
    }

    public sealed class FlightLoop : IDisposable
    {
        [StructLayout(LayoutKind.Sequential)]
        unsafe readonly struct Create
        {
            readonly int StructSize;
            readonly FlightLoopPhase Phase;
            readonly delegate* unmanaged<float, float, int, nint, float> Callback;
            readonly nint State;

            public Create(FlightLoopPhase phase, delegate* unmanaged<float, float, int, nint, float> callback, nint state)
            {
                (StructSize, Phase, State) = (sizeof(Create), phase, state);
                Callback = callback;
            }
        }

        readonly nint _id;

        readonly GCHandle _handle;

        public unsafe FlightLoop(FlightLoopPhase phase, Func<float, float, int, float> callback)
        {
            [DllImport(Defs.Lib)]
            static extern nint XPLMCreateFlightLoop(in Create options);

            [UnmanagedCallersOnly]
            static float Loop(float elapsedSinceLastCall, float elapsedSinceLastFlightLoop, int counter, nint handle) =>
                ((Func<float, float, int, float>)GCHandle.FromIntPtr(handle).Target!)
                    (elapsedSinceLastCall, elapsedSinceLastFlightLoop, counter);

            nint r = GCHandle.ToIntPtr(_handle = GCHandle.Alloc(callback));
            _id = XPLMCreateFlightLoop(new(phase, &Loop, r));
        }

        ~FlightLoop() => Dispose();

        bool _disposed;
        public void Dispose()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMDestroyFlightLoop(nint flightLoopId);

            if (!_disposed)
            {
                XPLMDestroyFlightLoop(_id);
                _handle.Free();
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        public void Schedule(float interval, bool relativeToNow)
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMScheduleFlightLoop(nint flightLoopId, float interval, int relativeToNow);

            XPLMScheduleFlightLoop(_id, interval, relativeToNow ? 1 : 0);
        }
    }
}