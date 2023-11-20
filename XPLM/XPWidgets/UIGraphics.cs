using System.Drawing;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPWidgets;

public static partial class UIGraphics
{
    public enum WindowStyle
    {
        Help,
        MainWindow,
        SubWindow,
        Screen,
        ListView
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPDrawWindow(int x1, int y1, int x2, int y2, WindowStyle style);

    public static void DrawWindow(Rectangle rectangle, WindowStyle style) =>
        XPDrawWindow(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, style);

    [LibraryImport(Defs.Lib)]
    private static partial void XPGetWindowDefaultDimensions(WindowStyle style, out int width, out int height);

    public static Size GetWindowDefaultDimensions(WindowStyle style)
    {
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

    [LibraryImport(Defs.Lib)]
    private static partial void XPDrawElement(int x1, int y1, int x2, int y2, ElementStyle style, int lit);

    public static void DrawElement(Rectangle rectangle, ElementStyle style, bool lit) =>
        XPDrawElement(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, style, lit ? 1 : 0);

    [LibraryImport(Defs.Lib)]
    private static partial void XPGetElementDefaultDimensions(ElementStyle style, out int width, out int height, out int canBeLit);

    public static Size GetElementDefaultDimensions(ElementStyle style, out bool canBeLit)
    {
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

    [LibraryImport(Defs.Lib)]
    private static partial void XPDrawTrack(int x1, int y1, int x2, int y2, int min, int max, int value, TrackStyle trackStyle, int lit);

    public static void DrawTrack(Rectangle rectangle, int min, int max, int value, TrackStyle trackStyle, bool lit) =>
        XPDrawTrack(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, min, max, value, trackStyle, lit ? 1 : 0);

    [LibraryImport(Defs.Lib)]
    private static partial void XPGetTrackDefaultDimensions(TrackStyle style, out int width, out int canBeLit);

    public static int GetTrackDefaultDimensions(TrackStyle style, out bool canBeLit)
    {
        XPGetTrackDefaultDimensions(style, out var width, out var c);
        canBeLit = c != 0;
        return width;
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPGetTrackMetrics(int x1, int y1, int x2, int y2, int min, int max, int value, TrackStyle trackStyle, out int isVertical, out int downBtnSize, out int downPageSize, out int thumbSize, out int upPageSize, out int upBtnSize);

    public static void GetTrackMetrics(Rectangle rectangle, int min, int max, int value, TrackStyle trackStyle, out bool isVertical, out int downBtnSize, out int downPageSize, out int thumbSize, out int upPageSize, out int upBtnSize)
    {
        XPGetTrackMetrics(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, min, max, value, trackStyle, out var v, out downBtnSize, out downPageSize, out thumbSize, out upPageSize, out upBtnSize);
        isVertical = v != 0;
    }
}