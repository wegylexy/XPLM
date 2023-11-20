using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;
public enum MenuCheck
{
    NoCheck,
    Unchecked,
    Checked
}

public sealed partial class Menu : IDisposable
{
    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMFindPluginsMenu();

    public static Menu Plugins => new(XPLMFindPluginsMenu());

    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMFindAircraftMenu();

    public static Menu Aircraft => new(XPLMFindAircraftMenu());

    [UnmanagedCallersOnly]
    static void H(nint menu, nint item)
    {
        var m = (Menu)GCHandle.FromIntPtr(menu).Target!;
        m._handler!(m, (Item)GCHandle.FromIntPtr(item).Target!);
    }

    readonly nint _id;

    readonly GCHandle? _handle;

    readonly Action<Menu, Item>? _handler;

    internal Menu(nint id) => _id = id;

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial nint XPLMCreateMenu(string name, nint parentMenu, int parentItem, delegate* unmanaged<nint, nint, void> handler, nint state);

    public unsafe Menu(string name, Menu? parentMenu, int parentItem, Action<Menu, Item>? handler = null)
    {
        _handle = GCHandle.Alloc(this);
        _handler = handler;
        _id = XPLMCreateMenu(name, parentMenu?._id ?? 0, parentItem, handler != null ? &H : null, GCHandle.ToIntPtr(_handle.Value));
    }

    ~Menu() => Dispose();

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDestroyMenu(nint menuID);

    bool _disposed;
    public void Dispose()
    {
        if (!_disposed)
        {
            XPLMDestroyMenu(_id);
            Clear();
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }

    void Clear()
    {
        foreach (var i in _items)
            i?._handle.Free();
        _items.Clear();
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMClearAllMenuItems(nint menuID);

    public void ClearAllItems()
    {
        XPLMClearAllMenuItems(_id);
        Clear();
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPLMAppendMenuItem(nint menu, string itemName, nint item, int _);

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial int XPLMAppendMenuItemWithCommand(nint menu, string itemName, nint commandToExecute);

    internal List<Item?> _items = [];
    public Item AppendItem(string name, Command? commandToExecute = null)
    {
        Item m = new(this);
        var h = m._handle;
        int i = commandToExecute is null
            ? XPLMAppendMenuItem(_id, name, GCHandle.ToIntPtr(h), 0)
            : XPLMAppendMenuItemWithCommand(_id, name, commandToExecute._id);
        if (i < 0)
        {
            h.Free();
            throw new InvalidOperationException();
        }
        _items.Insert(i, m);
        return m;
    }

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMAppendMenuSeparator(nint menu);

    public void AppendMenuSeparator()
    {
        var i = XPLMAppendMenuSeparator(_id);
        if (i < 0)
            throw new InvalidOperationException();
        _items.Insert(i, null);
    }

    public sealed partial class Item : IDisposable
    {
        readonly Menu _menu;

        internal readonly GCHandle _handle;

        int Index => _menu._items.IndexOf(this);

        internal Item(Menu menu) => (_menu, _handle) = (menu, GCHandle.Alloc(this));

        [LibraryImport(Defs.Lib)]
        private static partial void XPLMRemoveMenuItem(nint menu, int index);

        public void Dispose()
        {
            var i = Index;
            XPLMRemoveMenuItem(_menu._id, i);
            _menu._items.RemoveAt(i);
            _handle.Free();
        }

        [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
        private static partial void XPLMSetMenuItemName(nint menu, int index, string itemName, int _);

        public string Name
        {
            set => XPLMSetMenuItemName(_menu._id, Index, value, 0);
        }

        [LibraryImport(Defs.Lib)]
        private static partial void XPLMCheckMenuItem(nint menu, int index, MenuCheck check);

        [LibraryImport(Defs.Lib)]
        private static partial void XPLMCheckMenuItem(nint menu, int index, out MenuCheck check);

        public MenuCheck Check
        {
            set => XPLMCheckMenuItem(_menu._id, Index, value);
            get
            {
                XPLMCheckMenuItem(_menu._id, Index, out var check);
                return check;
            }
        }

        [LibraryImport(Defs.Lib)]
        private static partial void XPLMEnableMenuItem(nint menu, int index, int enabled);

        public bool Enabled
        {
            set => XPLMEnableMenuItem(_menu._id, Index, value ? 1 : 0);
        }
    }
}