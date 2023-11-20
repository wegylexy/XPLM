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

public abstract partial class Widget : IDisposable
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

    static readonly Dictionary<nint, Widget> _widgets = [];

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

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial nint XPCreateWidget(int left, int top, int right, int bottom, int visible, string descriptor, int isRoot, nint container, WidgetClass @class);

    [LibraryImport(Defs.Lib)]
    private static unsafe partial void XPAddWidgetCallback(nint widget, delegate* unmanaged<WidgetMessage, nint, nint, nint, int> newCallback);

    internal unsafe Widget(Rectangle rectangle, bool visible, string descriptor, Widget? container, WidgetClass @class)
    {
        _id = XPCreateWidget(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, visible ? 1 : 0, descriptor, container == null ? 1 : 0, container?._id ?? 0, @class);
        _widgets.Add(_id, this);
        XPAddWidgetCallback(_id, &Callback);
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial nint XPCreateCustomWidget(int left, int top, int right, int bottom, int visible, string descriptor, int isRoot, nint container, delegate* unmanaged<WidgetMessage, nint, nint, nint, int> callback);

    public unsafe Widget(Rectangle rectangle, bool visible, string descriptor, Widget? container) =>
        _widgets.Add(_id = XPCreateCustomWidget(rectangle.Left, rectangle.Top, rectangle.Right, rectangle.Bottom, visible ? 1 : 0, descriptor, container == null ? 1 : 0, container?._id ?? 0, &Callback), this);

    [LibraryImport(Defs.Lib)]
    private static partial void XPDestroyWidget(nint id, int destroyChildren);

    protected virtual void Destroy()
    {
        if (!_disposed)
        {
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

    [LibraryImport(Defs.Lib)]
    private static partial int XPSendMessageToWidget(nint widget, WidgetMessage message, DispatchMode mode, nint param1, nint param2);

    public bool SendMessage(WidgetMessage message, DispatchMode mode, nint param1, nint param2) =>
        XPSendMessageToWidget(_id, message, mode, param1, param2) != 0;

    [LibraryImport(Defs.Lib)]
    private static partial void XPPlaceWidgetWithin(nint subWidget, nint container);

    public void PlaceWithin(Widget? container) =>
        XPPlaceWidgetWithin(_id, container?._id ?? 0);

    partial class ChildList(Widget of) : IReadOnlyList<Widget>
    {
        readonly Widget _of = of;

        [LibraryImport(Defs.Lib)]
        private static partial nint XPGetNthChildWidget(nint widget, int index);

        public Widget this[int index] =>
            _widgets.TryGetValue(XPGetNthChildWidget(_of._id, index), out var w) ? w : throw new IndexOutOfRangeException();

        [LibraryImport(Defs.Lib)]
        private static partial int XPCountChildWidgets(nint widget);

        public int Count => XPCountChildWidgets(_of._id);

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

    [LibraryImport(Defs.Lib)]
    private static partial nint XPGetParentWidget(nint widget);

    public Widget? Parent => XPGetParentWidget(_id) is not 0 and var p ? _widgets[p] : null;

    [LibraryImport(Defs.Lib)]
    private static partial void XPShowWidget(nint widget);

    public void Show() => XPShowWidget(_id);

    [LibraryImport(Defs.Lib)]
    private static partial void XPHideWidget(nint widget);

    public void Hide() => XPHideWidget(_id);

    [LibraryImport(Defs.Lib)]
    private static partial int XPIsWidgetVisible(nint widget);

    public bool IsVisible => XPIsWidgetVisible(_id) != 0;

    [LibraryImport(Defs.Lib)]
    private static partial nint XPFindRootWidget(nint widget);

    public Widget? Root => XPFindRootWidget(_id) is not 0 and var p ? _widgets[p] : null;

    [LibraryImport(Defs.Lib)]
    private static partial void XPBringRootWidgetToFront(nint widget);

    public void BringRootToFront() => XPBringRootWidgetToFront(_id);

    [LibraryImport(Defs.Lib)]
    private static partial int XPIsWidgetInFront(nint widget);

    public bool IsInFront => XPIsWidgetInFront(_id) != 0;

    [LibraryImport(Defs.Lib)]
    private static partial void XPGetWidgetGeometry(nint widget, out int left, out int top, out int right, out int bottom);

    [LibraryImport(Defs.Lib)]
    private static partial void XPSetWidgetGeometry(nint widget, int left, int top, int right, int bottom);

    public Rectangle Geometry
    {
        get
        {
            XPGetWidgetGeometry(_id, out var left, out var top, out var right, out var bottom);
            return new(left, top, right - left, bottom - top);
        }
        set => XPSetWidgetGeometry(_id, value.Left, value.Top, value.Right, value.Bottom);
    }

    [LibraryImport(Defs.Lib)]
    private static partial nint XPGetWidgetForLocation(nint container, int xOffset, int yOffset, int recursive, int visibleOnly);

    public Widget? GetWidgetForLocation(Point offset, bool recursive, bool visibleOnly) =>
        XPGetWidgetForLocation(_id, offset.X, offset.Y, recursive ? 1 : 0, visibleOnly ? 1 : 0) is not 0 and var p ? _widgets[p] : null;

    [LibraryImport(Defs.Lib)]
    private static partial void XPGetWidgetExposedGeometry(nint widget, out int left, out int top, out int right, out int bottom);

    public Rectangle ExposedGeometry
    {
        get
        {
            XPGetWidgetExposedGeometry(_id, out var left, out var top, out var right, out var bottom);
            return new(left, top, right - left, bottom - top);
        }
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPSetWidgetDescriptor(nint widget, string descriptor);

    [LibraryImport(Defs.Lib)]
    private static partial int XPGetWidgetDescriptor(nint widget, in byte descriptor, int maxDescLength);

    public string Descriptor
    {
        set => XPSetWidgetDescriptor(_id, value);
        get
        {
            ReadOnlySpan<byte> d = stackalloc byte[XPGetWidgetDescriptor(_id, in Unsafe.NullRef<byte>(), 0)];
            _ = XPGetWidgetDescriptor(_id, in MemoryMarshal.GetReference(d), d.Length);
            return Encoding.UTF8.GetString(d);
        }
    }

    // TODO: get underlying window

    [LibraryImport(Defs.Lib)]
    private static partial void XPSetWidgetProperty(nint widget, int property, nint value);

    protected void SetProperty(int property, nint value) =>
        XPSetWidgetProperty(_id, property, value);

    [LibraryImport(Defs.Lib)]
    private static partial nint XPGetWidgetProperty(nint widget, int property, out int exists);

    protected nint GetProperty(int property, out bool exists)
    {
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

    [LibraryImport(Defs.Lib)]
    private static partial nint XPSetKeyboardFocus(nint widget);

    public Widget? SetKeyboardFocus() =>
        XPSetKeyboardFocus(_id) is not 0 and var p ? _widgets[p] : null;

    [LibraryImport(Defs.Lib)]
    private static partial void XPLoseKeyboardFocus(nint widget);

    public void LoseKeyboardFocus() =>
        XPLoseKeyboardFocus(_id);

    [LibraryImport(Defs.Lib)]
    private static partial nint XPGetWidgetWithFocus();

    public static Widget? WithFocus => XPGetWidgetWithFocus() is not 0 and var p ? _widgets[p] : null;

    [LibraryImport(Defs.Lib)]
    private static partial int XPUSelectIfNeeded(WidgetMessage message, nint widget, nint param1, nint param2, int eatClick);

    [LibraryImport(Defs.Lib)]
    private static partial int XPUDefocusKeyboard(WidgetMessage message, nint widget, nint param1, nint param2, int eatClick);

    [LibraryImport(Defs.Lib)]
    private static partial int XPUDragWidget(WidgetMessage message, nint widget, nint param1, nint param2, int left, int top, int right, int bottom);

    protected virtual bool Message(WidgetMessage message, nint param1, nint param2)
    {
        bool Drag() => DragRegion is { Left: var left, Top: var top, Right: var right, Bottom: var bottom } &&
            XPUDragWidget(message, _id, param1, param2, left, top, right, bottom) != 0;

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

    [LibraryImport(Defs.Lib)]
    private static partial void XPUMoveWidgetBy(nint widget, int deltaX, int deltaY);

    public void MoveBy(Point delta) =>
        XPUMoveWidgetBy(_id, delta.X, delta.Y);

    public bool? SelectIfNeeded { get; set; }

    public bool? DefocusKeyboard { get; set; }

    public Rectangle? DragRegion { get; set; }
}