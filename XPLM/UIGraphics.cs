using System.Drawing;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public static class UIGraphics
{
    public enum WindowStyle
    {
        Help,
        MainWindow,
        SubWindow,
        Screen,
        ListView
    }

    public static void DrawWindow(Rectangle rectangle, WindowStyle style)
    {
        [DllImport(Defs.Lib)]
        static extern void XPDrawWindow(int x1, int y1, int x2, int y2, WindowStyle style);

        XPDrawWindow(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, style);
    }

    public static Size GetWindowDefaultDimensions(WindowStyle style)
    {
        [DllImport(Defs.Lib)]
        static extern void XPGetWindowDefaultDimensions(WindowStyle style, out int width, out int height);

        XPGetWindowDefaultDimensions(style, out var width, out var height);
        return new(width, height);
    }

    public enum ElementStyle
    {
        TextField = 6,
        CheckBox = 9,
        CheckBoxLit = 10,
        WindowCloseBox = 14,
        WindowCloseBoxPressed,
        PushButton,
        PushButtonLit,
        OilPlatform = 24,
        OilPlatformSmall,
        Ship,
        ILSGlideScope,
        MarkerLeft,
        Airport,
        Waypoint,
        NDB,
        VOR,
        RadioTower,
        AircraftCarrier,
        Fire,
        MarkerRight,
        CustomObject,
        CoolingTower,
        SmokeStack,
        Building,
        PowerLine,
        CopyButtons = 45,
        CopyButtonsWithEditingGrid,
        EditingGrid,
        [Obsolete("Draw track instead.")] ScrollBar,
        VORWithCompassRose,
        Zoomer = 51,
        TextFieldMiddle,
        LittleDownArrow,
        LittleUpArrow,
        WindowDragBar = 61,
        WindowDragBarSmooth
    }

    public static void DrawElement(Rectangle rectangle, ElementStyle style, bool lit)
    {
        [DllImport(Defs.Lib)]
        static extern void XPDrawElement(int x1, int y1, int x2, int y2, ElementStyle style, int lit);

        XPDrawElement(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, style, lit ? 1 : 0);
    }

    public static Size GetElementDefaultDimensions(ElementStyle style, out bool canBeLit)
    {
        [DllImport(Defs.Lib)]
        static extern void XPGetElementDefaultDimensions(ElementStyle style, out int width, out int height, out int canBeLit);

        XPGetElementDefaultDimensions(style, out var width, out var height, out var c);
        canBeLit = c != 0;
        return new(width, height);
    }

    public enum TrackStyle
    {
        ScrollBar,
        Slider,
        Progress
    }

    public static void DrawTrack(Rectangle rectangle, int min, int max, int value, TrackStyle trackStyle, bool lit)
    {
        [DllImport(Defs.Lib)]
        static extern void XPDrawTrack(int x1, int y1, int x2, int y2, int min, int max, int value, TrackStyle trackStyle, int lit);

        XPDrawTrack(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, min, max, value, trackStyle, lit ? 1 : 0);
    }

    public static int GetTrackDefaultDimensions(TrackStyle style, out bool canBeLit)
    {
        [DllImport(Defs.Lib)]
        static extern void XPGetTrackDefaultDimensions(TrackStyle style, out int width, out int canBeLit);

        XPGetTrackDefaultDimensions(style, out var width, out var c);
        canBeLit = c != 0;
        return width;
    }

    public static void GetTrackMetrics(Rectangle rectangle, int min, int max, int value, TrackStyle trackStyle, out bool isVertical, out int downBtnSize, out int downPageSize, out int thumbSize, out int upPageSize, out int upBtnSize)
    {
        [DllImport(Defs.Lib)]
        static extern void XPGetTrackMetrics(int x1, int y1, int x2, int y2, int min, int max, int value, TrackStyle trackStyle, out int isVertical, out int downBtnSize, out int downPageSize, out int thumbSize, out int upPageSize, out int upBtnSize);

        XPGetTrackMetrics(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, min, max, value, trackStyle, out var v, out downBtnSize, out downPageSize, out thumbSize, out upPageSize, out upBtnSize);
        isVertical = v != 0;
    }
}