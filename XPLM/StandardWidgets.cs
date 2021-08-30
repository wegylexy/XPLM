using System.Drawing;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM.StandardWidgets;

public enum MainWindowStyle
{
    MainWindow,
    Translucent
}

public sealed class MainWindow : Widget
{
    public event Func<MainWindow, bool>? CloseButtonPushed;

    public MainWindow(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.MainWindow)
    { }

    public MainWindowStyle Style
    {
        get => (MainWindowStyle)GetProperty(1100, out _);
        set => SetProperty(1100, (nint)value);
    }

    public bool HasCloseBoxes
    {
        get => GetProperty(1200, out _) != 0;
        set => SetProperty(1200, value ? 1 : 0);
    }

    protected override bool Message(WidgetMessage message, nint param1, nint param2)
    {
        [DllImport(Defs.Lib)]
        static extern int XPUFixedLayout(WidgetMessage message, nint widget, nint param1, nint param2);

        return message switch
        {
            WidgetMessage.CloseButtonPushed => CloseButtonPushed?.Invoke(this),
            WidgetMessage.Reshape when FixedLayout => XPUFixedLayout(message, _id, param1, param2) != 0,
            _ => base.Message(message, param1, param2)
        } is true;
    }

    public bool FixedLayout { get; set; }
}

public enum SubWindowStyle
{
    SubWindow,
    Screen = 2,
    ListView
}

public sealed class SubWindow : Widget
{
    public SubWindow(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.SubWindow)
    { }

    public SubWindowStyle Style
    {
        get => (SubWindowStyle)GetProperty(1200, out _);
        set => SetProperty(1200, (nint)value);
    }
}

public enum ButtonStyle
{
    PushButton,
    RadioButton,
    WindowCloseBox = 3,
    LittleDownArrow = 5,
    LittleUpArrow
}

public enum ButtonBehavior
{
    PushButton,
    CheckBox,
    RadioButton
}

public sealed class Button : Widget
{
    public event Func<Button, bool>? Pressed;
    public event Func<Button, bool, bool>? StateChanged;

    public Button(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.Button)
    { }

    public ButtonStyle Style
    {
        get => (ButtonStyle)GetProperty(1300, out _);
        set => SetProperty(1300, (nint)value);
    }

    public ButtonBehavior Behavior
    {
        get => (ButtonBehavior)GetProperty(1301, out _);
        set => SetProperty(1301, (nint)value);
    }

    public bool State
    {
        get => GetProperty(1302, out _) != 0;
        set => SetProperty(1302, value ? 1 : 0);
    }

    protected override bool Message(WidgetMessage message, nint param1, nint param2) =>
        message switch
        {
            WidgetMessage.PushButtonPressed => Pressed?.Invoke(this),
            WidgetMessage.ButtonStateChanged => StateChanged?.Invoke(this, param2 != 0),
            _ => base.Message(message, param1, param2)
        } is true;
}

public enum TextFieldStyle
{
    EntryField,
    Transparent = 3,
    Translucent = 4
}

public sealed class TextField : Widget
{
    public event Func<TextField, bool>? Changed;

    public TextField(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.TextField)
    { }

    public int SelectionStart
    {
        get => (int)GetProperty(1400, out _);
        set => SetProperty(1400, value);
    }

    public int SelectionEnd
    {
        get => (int)GetProperty(1401, out _);
        set => SetProperty(1401, value);
    }

    public int SelectionDragStart
    {
        get => (int)GetProperty(1402, out _);
        set => SetProperty(1402, value);
    }

    public TextFieldStyle Style
    {
        get => (TextFieldStyle)GetProperty(1403, out _);
        set => SetProperty(1403, (nint)value);
    }

    public bool PasswordMode
    {
        get => GetProperty(1404, out _) != 0;
        set => SetProperty(1404, value ? 1 : 0);
    }

    public int MaxCharacters
    {
        get => (int)GetProperty(1405, out _);
        set => SetProperty(1405, value);
    }

    public int ScrollPosition
    {
        get => (int)GetProperty(1406, out _);
        set => SetProperty(1406, value);
    }

    public Text.FontID Font
    {
        get => (Text.FontID)GetProperty(1407, out _);
        set => SetProperty(1407, (nint)value);
    }

    protected override bool Message(WidgetMessage message, nint param1, nint param2) =>
        message == WidgetMessage.TextFieldChanged ?
            Changed?.Invoke(this) is true :
            base.Message(message, param1, param2);
}

public enum ScrollBarStyle
{
    ScrollBar,
    Slider
}

public sealed class ScrollBar : Widget
{
    public event Func<ScrollBar, bool>? Changed;

    public ScrollBar(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.ScrollBar)
    { }

    public int SliderPosition
    {
        get => (int)GetProperty(1500, out _);
        set => SetProperty(1500, value);
    }

    public int Min
    {
        get => (int)GetProperty(1501, out _);
        set => SetProperty(1501, value);
    }

    public int Max
    {
        get => (int)GetProperty(1502, out _);
        set => SetProperty(1502, value);
    }

    public int PageAmount
    {
        get => (int)GetProperty(1503, out _);
        set => SetProperty(1503, value);
    }

    public ScrollBarStyle Style
    {
        get => (ScrollBarStyle)GetProperty(1504, out _);
        set => SetProperty(1504, (nint)value);
    }

    protected override bool Message(WidgetMessage message, nint param1, nint param2) =>
        message == WidgetMessage.ScrollBarSliderPositionChanged ?
            Changed?.Invoke(this) is true :
            base.Message(message, param1, param2);
}

public sealed class Caption : Widget
{
    public Caption(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.Caption)
    { }

    public bool Lit
    {
        get => GetProperty(1600, out _) != 0;
        set => SetProperty(1600, value ? 1 : 0);
    }
}

public enum GeneralGraphicsStyle
{
    Ship = 4,
    ILSGlideScope,
    MarkerLeft,
    Airport,
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
    VORWithCompassRose,
    OilPlatform = 21,
    OilPlatformSmall,
    WayPoint
}

public sealed class GeneralGraphics : Widget
{
    public GeneralGraphics(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.GeneralGraphics)
    { }

    public GeneralGraphicsStyle Style
    {
        get => (GeneralGraphicsStyle)GetProperty(1700, out _);
        set => SetProperty(1700, (nint)value);
    }
}

public sealed class Progress : Widget
{
    public Progress(Rectangle rectangle, bool visible, string descriptor) :
        base(rectangle, visible, descriptor, null, WidgetClass.Progress)
    { }

    public int Position
    {
        get => (int)GetProperty(1800, out _);
        set => SetProperty(1800, value);
    }

    public int Min
    {
        get => (int)GetProperty(1801, out _);
        set => SetProperty(1801, value);
    }

    public int Max
    {
        get => (int)GetProperty(1802, out _);
        set => SetProperty(1802, value);
    }
}