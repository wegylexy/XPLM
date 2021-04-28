using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM
{
    public enum MenuCheck
    {
        NoCheck,
        Unchecked,
        Checked
    }

    public sealed class Menu : IDisposable
    {
        [UnmanagedCallersOnly]
        static void H(nint menu, nint item)
        {
            var m = (Menu)GCHandle.FromIntPtr(menu).Target!;
            m._handler!(m, (MenuItem)GCHandle.FromIntPtr(item).Target!);
        }

        internal readonly nint _id;

        readonly GCHandle? _handle;

        readonly Action<Menu, MenuItem>? _handler;

        internal Menu(nint id) => _id = id;

        public unsafe Menu(string name, Menu? parentMenu, int parentItem, Action<Menu, MenuItem>? handler = null)
        {
            [DllImport(Defs.Lib)]
            static extern unsafe nint XPLMCreateMenu([MarshalAs(UnmanagedType.LPUTF8Str)] string name, nint parentMenu, int parentItem, delegate* unmanaged<nint, nint, void> handler, nint state);

            _handle = GCHandle.Alloc(this);
            _id = XPLMCreateMenu(name, parentMenu?._id ?? 0, parentItem, handler != null ? &H : null, GCHandle.ToIntPtr(_handle.Value));
        }

        ~Menu() => Dispose();

        bool _disposed;
        public void Dispose()
        {
            if (!_disposed)
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMDestroyMenu(nint menuID);

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

        public void ClearAllItems()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMClearAllMenuItems(nint menuID);

            XPLMClearAllMenuItems(_id);
            Clear();
        }

        internal List<MenuItem?> _items = new();
        public MenuItem AppendItem(string name, Command? commandToExecute = null)
        {
            MenuItem m = new(this);
            var h = m._handle;
            int i;
            if (commandToExecute is null)
            {
                [DllImport(Defs.Lib)]
                static extern int XPLMAppendMenuItem(nint menu, [MarshalAs(UnmanagedType.LPUTF8Str)] string itemName, nint item, int _);

                i = XPLMAppendMenuItem(_id, name, GCHandle.ToIntPtr(h), 0);
            }
            else
            {
                [DllImport(Defs.Lib)]
                static extern int XPLMAppendMenuItemWithCommand(nint menu, [MarshalAs(UnmanagedType.LPUTF8Str)] string itemName, nint commandToExecute);

                i = XPLMAppendMenuItemWithCommand(_id, name, commandToExecute._id);
            }
            if (i < 0)
            {
                h.Free();
                throw new InvalidOperationException();
            }
            _items.Insert(i, m);
            return m;
        }

        public void AppendMenuSeparator()
        {
            [DllImport(Defs.Lib)]
            static extern int XPLMAppendMenuSeparator(nint menu);

            var i = XPLMAppendMenuSeparator(_id);
            if (i < 0)
                throw new InvalidOperationException();
            _items.Insert(i, null);
        }
    }

    public sealed class MenuItem : IDisposable
    {
        readonly Menu _menu;

        internal readonly GCHandle _handle;

        int Index => _menu._items.IndexOf(this);

        internal MenuItem(Menu menu) => (_menu, _handle) = (menu, GCHandle.Alloc(this));

        public void Dispose()
        {
            [DllImport(Defs.Lib)]
            static extern void XPLMRemoveMenuItem(nint menu, int index);

            var i = Index;
            XPLMRemoveMenuItem(_menu._id, i);
            _menu._items.RemoveAt(i);
            _handle.Free();
        }

        public string Name
        {
            set
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMSetMenuItemName(nint menu, int index, [MarshalAs(UnmanagedType.LPUTF8Str)] string itemName, int _);

                XPLMSetMenuItemName(_menu._id, Index, value, 0);
            }
        }

        public MenuCheck Check
        {
            set
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMCheckMenuItem(nint menu, int index, MenuCheck check);

                XPLMCheckMenuItem(_menu._id, Index, value);
            }
            get
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMCheckMenuItem(nint menu, int index, out MenuCheck check);

                XPLMCheckMenuItem(_menu._id, Index, out var check);
                return check;
            }
        }

        public bool Enabled
        {
            set
            {
                [DllImport(Defs.Lib)]
                static extern void XPLMEnableMenuItem(nint menu, int index, int enabled);

                XPLMEnableMenuItem(_menu._id, Index, value ? 1 : 0);
            }
        }
    }

    public static class Menus
    {
        public static Menu Plugins
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern nint XPLMFindPluginsMenu();

                return new(XPLMFindPluginsMenu());
            }
        }

        public static Menu Aircraft
        {
            get
            {
                [DllImport(Defs.Lib)]
                static extern nint XPLMFindAircraftMenu();

                return new(XPLMFindAircraftMenu());
            }
        }
    }
}