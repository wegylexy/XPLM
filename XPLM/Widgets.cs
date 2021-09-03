using FlyByWireless.XPLM.Display;
using System.Collections;
using System.Drawing;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace FlyByWireless.XPLM;

[StructLayout(LayoutKind.Sequential)]
public readonly struct MouseState
{
    public readonly int X, Y, Button, Delta;
}

[StructLayout(LayoutKind.Sequential)]
public readonly struct KeyState
{
    public readonly byte Key;
    public readonly KeyFlags Flags;
    public readonly byte VKey;
}

[StructLayout(LayoutKind.Sequential)]
public readonly struct WidgetGeometryChange
{
    public readonly int DX, DY, DWidth, DHeight;
}

public enum DispatchMode
{
    Direct,
    UpChain,
    Recursive,
    DirectAllCallbacks,
    Once
}

public enum WidgetMessage
{
    None,
    Create,
    Destroy,
    Paint,
    Draw,
    KeyPress,
    KeyTakeFocus,
    KeyLoseFocus,
    MouseDown,
    MouseDrag,
    MouseUp,
    Reshape,
    ExposedChanged,
    AcceptChild,
    LoseChild,
    AcceptParent,
    Shown,
    Hidden,
    DescriptorChanged,
    PropertyChanged,
    MouseWheel,
    CursorAdjust,
    CloseButtonPushed = 1200,
    PushButtonPressed = 1300,
    ButtonStateChanged,
    TextFieldChanged = 1400,
    ScrollBarSliderPositionChanged = 1500,
    UserStart = 10000
}

public abstract class Widget : IDisposable
{
    internal enum WidgetClass
    {
        MainWindow = 1,
        SubWindow,
        Button,
        TextField,
        ScrollBar,
        Caption,
        GeneralGraphics,
        Progress
    }

    static readonly Dictionary<nint, Widget> _widgets = new();

    protected readonly nint _id;
    bool _disposed;

    public event Func<Widget, bool>? Paint, Draw;

    public event Func<Widget, KeyState, bool>? KeyPress;

    public event Func<Widget, bool, bool>? KeyTakeFocus, KeyLoseFocus;

    public event Func<Widget, MouseState, bool>? MouseDown, MouseDrag, MouseUp;

    public event Func<Widget, Widget, WidgetGeometryChange, bool>? Reshape;

    public event Func<Widget, bool>? ExposedChanged;

    public event Func<Widget, Widget, bool>? AcceptChild, LoseChild, AcceptParent, Shown, Hidden;

    public event Func<Widget, bool>? DescriptorChanged;

    protected event Func<Widget, int, nint, bool>? PropertyChanged;

    public event Func<Widget, MouseState, bool>? MouseWheel;

    public event Func<Widget, MouseState, CursorStatus, bool>? CursorAdjust;

    internal unsafe Widget(Rectangle rectangle, bool visible, string descriptor, Widget? container, WidgetClass @class)
    {
        [DllImport(Defs.Lib)]
        static extern nint XPCreateWidget(int left, int top, int right, int bottom, int visible, [MarshalAs(UnmanagedType.LPUTF8Str)] string descriptor, int isRoot, nint container, WidgetClass @class);

        [DllImport(Defs.Lib)]
        static extern void XPAddWidgetCallback(nint widget, delegate* unmanaged<WidgetMessage, nint, nint, nint, int> newCallback);

        _id = XPCreateWidget(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, visible ? 1 : 0, descriptor, container == null ? 1 : 0, container?._id ?? 0, @class);
        _widgets.Add(_id, this);
        XPAddWidgetCallback(_id, &Callback);
    }

    public unsafe Widget(Rectangle rectangle, bool visible, string descriptor, Widget? container)
    {
        [DllImport(Defs.Lib)]
        static extern nint XPCreateCustomWidget(int left, int top, int right, int bottom, int visible, [MarshalAs(UnmanagedType.LPUTF8Str)] string descriptor, int isRoot, nint container, delegate* unmanaged<WidgetMessage, nint, nint, nint, int> callback);

        _widgets.Add(_id = XPCreateCustomWidget(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, visible ? 1 : 0, descriptor, container == null ? 1 : 0, container?._id ?? 0, &Callback), this);
    }

    protected virtual void Destroy()
    {
        if (!_disposed)
        {
            [DllImport(Defs.Lib)]
            static extern void XPDestroyWidget(nint id, int destroyChildren);

            XPDestroyWidget(_id, 0);
            _ = _widgets.Remove(_id);
            _disposed = true;
        }
    }

    ~Widget() => Destroy();

    public void Dispose()
    {
        Destroy();
        GC.SuppressFinalize(this);
    }

    public bool SendMessage(WidgetMessage message, DispatchMode mode, nint param1, nint param2)
    {
        [DllImport(Defs.Lib)]
        static extern int XPSendMessageToWidget(nint widget, WidgetMessage message, DispatchMode mode, nint param1, nint param2);

        return XPSendMessageToWidget(_id, message, mode, param1, param2) != 0;
    }

    public void PlaceWithin(Widget? container)
    {
        [DllImport(Defs.Lib)]
        static extern void XPPlaceWidgetWithin(nint subWidget, nint container);

        XPPlaceWidgetWithin(_id, container?._id ?? 0);
    }

    class ChildList : IReadOnlyList<Widget>
    {
        readonly Widget _of;

        public ChildList(Widget of) => _of = of;

        public Widget this[int index]
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern nint XPGetNthChildWidget(nint widget, int index);

                if (_widgets.TryGetValue(XPGetNthChildWidget(_of._id, index), out var w))
                {
                    return w;
                }
                else
                {
                    throw new IndexOutOfRangeException();
                }
            }
        }

        public int Count
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern int XPCountChildWidgets(nint widget);

                return XPCountChildWidgets(_of._id);
            }
        }

        public IEnumerator<Widget> GetEnumerator()
        {
            for (var i = 0; i < Count; ++i)
            {
                yield return this[i];
            }
        }

        IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();
    }

    IReadOnlyList<Widget>? _Children;
    public IReadOnlyList<Widget> Children => _Children ??= new ChildList(this);

    public Widget? Parent
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern nint XPGetParentWidget(nint widget);

            return XPGetParentWidget(_id) is not 0 and var p ? _widgets[p] : null;
        }
    }

    public void Show()
    {
        [DllImport(Defs.Lib)]
        static extern void XPShowWidget(nint widget);

        XPShowWidget(_id);
    }

    public void Hide()
    {
        [DllImport(Defs.Lib)]
        static extern void XPHideWidget(nint widget);

        XPHideWidget(_id);
    }

    public bool IsVisible
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPIsWidgetVisible(nint widget);

            return XPIsWidgetVisible(_id) != 0;
        }
    }

    public Widget? Root
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern nint XPFindRootWidget(nint widget);

            return XPFindRootWidget(_id) is not 0 and var p ? _widgets[p] : null;
        }
    }

    public void BringRootToFront()
    {
        [DllImport(Defs.Lib)]
        static extern void XPBringRootWidgetToFront(nint widget);

        XPBringRootWidgetToFront(_id);
    }

    public bool IsInFront
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPIsWidgetInFront(nint widget);

            return XPIsWidgetInFront(_id) != 0;
        }
    }

    public Rectangle Geometry
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern void XPGetWidgetGeometry(nint widget, out int left, out int top, out int right, out int bottom);

            XPGetWidgetGeometry(_id, out var left, out var top, out var right, out var bottom);
            return new(left, top, right - left, bottom - top);
        }
        set
        {
            [DllImport(Defs.Lib)]
            static extern void XPSetWidgetGeometry(nint widget, int left, int top, int right, int bottom);

            XPSetWidgetGeometry(_id, value.Left, value.Top, value.Right, value.Bottom);
        }
    }

    public Widget? GetWidgetForLocation(Point offset, bool recursive, bool visibleOnly)
    {
        [DllImport(Defs.Lib)]
        static extern nint XPGetWidgetForLocation(nint container, int xOffset, int yOffset, int recursive, int visibleOnly);

        return XPGetWidgetForLocation(_id, offset.X, offset.Y, recursive ? 1 : 0, visibleOnly ? 1 : 0) is not 0 and var p ? _widgets[p] : null;
    }

    public Rectangle ExposedGeometry
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern void XPGetWidgetExposedGeometry(nint widget, out int left, out int top, out int right, out int bottom);

            XPGetWidgetExposedGeometry(_id, out var left, out var top, out var right, out var bottom);
            return new(left, top, right - left, bottom - top);
        }
    }

    public string Descriptor
    {
        set
        {
            [DllImport(Defs.Lib)]
            static extern void XPSetWidgetDescriptor(nint widget, [MarshalAs(UnmanagedType.LPUTF8Str)] string descriptor);

            XPSetWidgetDescriptor(_id, value);
        }
        get
        {
            [DllImport(Defs.Lib)]
            static extern int XPGetWidgetDescriptor(nint widget, in byte descriptor, int maxDescLength);

            ReadOnlySpan<byte> d = stackalloc byte[XPGetWidgetDescriptor(_id, in Unsafe.NullRef<byte>(), 0)];
            _ = XPGetWidgetDescriptor(_id, in MemoryMarshal.GetReference(d), d.Length);
            return Encoding.UTF8.GetString(d);
        }
    }

    // TODO: get underlying window

    protected void SetProperty(int property, nint value)
    {
        [DllImport(Defs.Lib)]
        static extern void XPSetWidgetProperty(nint widget, int property, nint value);

        XPSetWidgetProperty(_id, property, value);
    }

    protected nint GetProperty(int property, out bool exists)
    {
        [DllImport(Defs.Lib)]
        static extern nint XPGetWidgetProperty(nint widget, int property, out int exists);

        var p = XPGetWidgetProperty(_id, property, out var e);
        exists = e != 0;
        return p;
    }

    public bool Hilited
    {
        get => GetProperty(4, out _) != 0;
        set => SetProperty(4, value ? 1 : 0);
    }

    public bool Clip
    {
        get => GetProperty(6, out _) != 0;
        set => SetProperty(6, value ? 1 : 0);
    }

    public bool Enabled
    {
        get => GetProperty(7, out _) != 0;
        set => SetProperty(7, value ? 1 : 0);
    }

    public Widget? SetKeyboardFocus()
    {
        [DllImport(Defs.Lib)]
        static extern nint XPSetKeyboardFocus(nint widget);

        return XPSetKeyboardFocus(_id) is not 0 and var p ? _widgets[p] : null;
    }

    public void LoseKeyboardFocus()
    {
        [DllImport(Defs.Lib)]
        static extern void XPLoseKeyboardFocus(nint widget);

        XPLoseKeyboardFocus(_id);
    }

    public static Widget? WithFocus
    {
        get
        {
            [DllImport(Defs.Lib)]
            static extern nint XPGetWidgetWithFocus();

            return XPGetWidgetWithFocus() is not 0 and var p ? _widgets[p] : null;
        }
    }

    protected virtual bool Message(WidgetMessage message, nint param1, nint param2)
    {
        [DllImport(Defs.Lib)]
        static extern int XPUSelectIfNeeded(WidgetMessage message, nint widget, nint param1, nint param2, int eatClick);

        [DllImport(Defs.Lib)]
        static extern int XPUDefocusKeyboard(WidgetMessage message, nint widget, nint param1, nint param2, int eatClick);

        bool Drag()
        {
            [DllImport(Defs.Lib)]
            static extern int XPUDragWidget(WidgetMessage message, nint widget, nint param1, nint param2, int left, int top, int right, int bottom);

            return DragRegion is { Left: var left, Top: var top, Right: var right, Bottom: var bottom } &&
                XPUDragWidget(message, _id, param1, param2, left, top, right, bottom) != 0;
        }

        return message switch
        {
            WidgetMessage.Destroy => _disposed = _widgets.Remove(_id),
            WidgetMessage.Paint => Paint?.Invoke(this),
            WidgetMessage.Draw => Draw?.Invoke(this),
            WidgetMessage.KeyPress => KeyPress?.Invoke(this, Marshal.PtrToStructure<KeyState>(param1)),
            WidgetMessage.KeyTakeFocus => KeyTakeFocus?.Invoke(this, param1 != 0),
            WidgetMessage.KeyLoseFocus => KeyLoseFocus?.Invoke(this, param1 != 0),
            WidgetMessage.MouseDown when !(
                SelectIfNeeded.HasValue && XPUSelectIfNeeded(message, _id, param1, param2, SelectIfNeeded.Value ? 1 : 0) != 0 ||
                DefocusKeyboard.HasValue && XPUDefocusKeyboard(message, _id, param1, param2, DefocusKeyboard.Value ? 1 : 0) != 0 ||
                Drag()
            ) => MouseDown?.Invoke(this, Marshal.PtrToStructure<MouseState>(param1)),
            WidgetMessage.MouseDrag when !Drag() => MouseDrag?.Invoke(this, Marshal.PtrToStructure<MouseState>(param1)),
            WidgetMessage.MouseUp when !Drag() => MouseUp?.Invoke(this, Marshal.PtrToStructure<MouseState>(param1)),
            WidgetMessage.Reshape => Reshape?.Invoke(this, _widgets.TryGetValue(param1, out var p) ? p : null!, Marshal.PtrToStructure<WidgetGeometryChange>(param2)),
            WidgetMessage.ExposedChanged => ExposedChanged?.Invoke(this),
            WidgetMessage.AcceptChild => AcceptChild?.Invoke(this, _widgets.TryGetValue(param1, out var p) ? p : null!),
            WidgetMessage.LoseChild => LoseChild?.Invoke(this, _widgets.TryGetValue(param1, out var p) ? p : null!),
            WidgetMessage.AcceptParent => AcceptParent?.Invoke(this, _widgets.TryGetValue(param1, out var p) ? p : null!),
            WidgetMessage.Shown => Shown?.Invoke(this, _widgets.TryGetValue(param1, out var p) ? p : null!),
            WidgetMessage.Hidden => Hidden?.Invoke(this, _widgets.TryGetValue(param1, out var p) ? p : null!),
            WidgetMessage.DescriptorChanged => DescriptorChanged?.Invoke(this),
            WidgetMessage.PropertyChanged => PropertyChanged?.Invoke(this, (int)param1, param2),
            WidgetMessage.MouseWheel => MouseWheel?.Invoke(this, Marshal.PtrToStructure<MouseState>(param1)),
            WidgetMessage.CursorAdjust => CursorAdjust?.Invoke(this, Marshal.PtrToStructure<MouseState>(param1), (CursorStatus)param2),
            _ => default,
        } is true;
    }

    [UnmanagedCallersOnly]
    static int Callback(WidgetMessage message, nint widget, nint param1, nint param2) =>
        _widgets.TryGetValue(widget, out var p) && p.Message(message, param1, param2) is true ? 1 : 0;

    public void MoveBy(Point delta)
    {
        [DllImport(Defs.Lib)]
        static extern void XPUMoveWidgetBy(nint widget, int deltaX, int deltaY);

        XPUMoveWidgetBy(_id, delta.X, delta.Y);
    }

    public bool? SelectIfNeeded { get; set; }

    public bool? DefocusKeyboard { get; set; }

    public Rectangle? DragRegion { get; set; }
}