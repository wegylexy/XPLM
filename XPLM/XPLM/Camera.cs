using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public enum CameraControlDuration
{
    Uncontrolled,
    UntilViewChanges,
    Forever
}

public readonly struct CameraPosition
{
    public readonly float X, Y, Z, Pitch, Heading, Roll, Zoom;

    public CameraPosition(float x, float y, float z, float pitch, float heading, float roll, float zoom) =>
        (X, Y, Z, Pitch, Heading, Roll, Zoom) = (x, y, z, pitch, heading, roll, zoom);
}

public delegate bool CameraControl(out CameraPosition? cameraPosition, bool isLosingControl);

public static partial class Camera
{
    static GCHandle? _handle;

    [LibraryImport(Defs.Lib)]
    private static unsafe partial void XPLMControlCamera(CameraControlDuration howLong, delegate* unmanaged<CameraPosition*, int, nint, int> control, nint state);

    public static unsafe void Control(CameraControlDuration howLong, CameraControl control)
    {
        [UnmanagedCallersOnly]
        static int C(CameraPosition* cameraPosition, int isLosingControl, nint state)
        {
            var c = ((CameraControl)GCHandle.FromIntPtr(state).Target!)(out var position, isLosingControl != 0);
            if (position.HasValue && cameraPosition != null)
                *cameraPosition = position!.Value;
            return c ? 1 : 0;
        }

        if (_handle.HasValue)
            _handle.Value.Free();
        _handle = GCHandle.Alloc(control);
        XPLMControlCamera(howLong, &C, GCHandle.ToIntPtr(_handle.Value));
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDontControlCamera();

    public static void DontControl()
    {
        XPLMDontControlCamera();
        if (_handle.HasValue)
        {
            _handle.Value.Free();
            _handle = null;
        }
    }

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMIsCameraBeingControlled(out CameraControlDuration duration);

    public static CameraControlDuration IsBeingControlled =>
        XPLMIsCameraBeingControlled(out var duration) != 0 ? duration : default;

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMReadCameraPosition(out CameraPosition cameraPosition);

    public static CameraPosition Position
    {
        get
        {
            XPLMReadCameraPosition(out var position);
            return position;
        }
    }
}