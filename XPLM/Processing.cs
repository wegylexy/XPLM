using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM.Processing;

public enum FlightLoopPhase
{
    BeforeFlightModel,
    AfterFlightModel
}

public sealed partial class FlightLoop : IDisposable
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

    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMCreateFlightLoop(in Create options);

    public unsafe FlightLoop(FlightLoopPhase phase, Func<float, float, int, float> callback)
    {
        [UnmanagedCallersOnly]
        static float Loop(float elapsedSinceLastCall, float elapsedSinceLastFlightLoop, int counter, nint handle)
        {
            try
            {
                return ((Func<float, float, int, float>)GCHandle.FromIntPtr(handle).Target!)
                    (elapsedSinceLastCall, elapsedSinceLastFlightLoop, counter);
            }
            catch (Exception ex)
            {
                Utilities.DebugString(ex.ToString() + "\n");
                return 0;
            }
        }

        nint r = GCHandle.ToIntPtr(_handle = GCHandle.Alloc(callback));
        _id = XPLMCreateFlightLoop(new(phase, &Loop, r));
    }

    ~FlightLoop() => Dispose();

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDestroyFlightLoop(nint flightLoopId);

    bool _disposed;
    public void Dispose()
    {
        if (!_disposed)
        {
            XPLMDestroyFlightLoop(_id);
            _handle.Free();
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMScheduleFlightLoop(nint flightLoopId, float interval, int relativeToNow);

    public void Schedule(float interval, bool relativeToNow) =>
        XPLMScheduleFlightLoop(_id, interval, relativeToNow ? 1 : 0);
}