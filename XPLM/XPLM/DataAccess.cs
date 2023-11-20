using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;

namespace FlyByWireless.XPLM.DataAccess;

[Flags]
public enum DataTypes
{
    Unknown = 0,
    Int = 1,
    Float = 2,
    Double = 4,
    FloatArray = 8,
    IntArray = 16,
    Data = 32
}

public sealed partial class IntVector
{
    [LibraryImport(Defs.Lib)]
    private static partial int XPLMGetDatavi(nint handle, ref int values, int offset, int max);

    readonly nint _handle;

    readonly int _offset;

    public int Count => XPLMGetDatavi(_handle, ref Unsafe.NullRef<int>(), 0, 0);

    internal IntVector(nint handle, int offset) => (_handle, _offset) = (handle, offset);

    public int Read(Span<int> destination) =>
        XPLMGetDatavi(_handle, ref MemoryMarshal.GetReference(destination), _offset, destination.Length);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSetDatavi(nint handle, ref int values, int offset, int count);

    public void Write(ReadOnlySpan<int> source)
    {
        XPLMSetDatavi(_handle, ref MemoryMarshal.GetReference(source), _offset, source.Length);
    }
}

public sealed partial class FloatVector
{
    [LibraryImport(Defs.Lib)]
    private static partial int XPLMGetDatavf(nint handle, ref float values, int offset, int max);

    readonly nint _handle;

    readonly int _offset;

    public int Count => XPLMGetDatavf(_handle, ref Unsafe.NullRef<float>(), 0, 0);

    internal FloatVector(nint handle, int offset) => (_handle, _offset) = (handle, offset);

    public int Read(Span<float> destination) =>
        XPLMGetDatavf(_handle, ref MemoryMarshal.GetReference(destination), _offset, destination.Length);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSetDatavf(nint handle, ref float values, int offset, int count);

    public void Write(ReadOnlySpan<float> source)
    {
        XPLMSetDatavf(_handle, ref MemoryMarshal.GetReference(source), _offset, source.Length);
    }
}

public sealed partial class ByteVector
{
    [LibraryImport(Defs.Lib)]
    internal static partial int XPLMGetDatab(nint handle, ref byte values, int offset, int max);

    [LibraryImport(Defs.Lib)]
    internal static partial void XPLMSetDatab(nint handle, ref byte values, int offset, int count);

    readonly nint _handle;

    readonly int _offset;

    public int Count => XPLMGetDatab(_handle, ref Unsafe.NullRef<byte>(), 0, 0);

    internal ByteVector(nint handle, int offset) => (_handle, _offset) = (handle, offset);

    public int Read(Span<byte> destination) =>
        XPLMGetDatab(_handle, ref MemoryMarshal.GetReference(destination), _offset, destination.Length);

    public void Write(ReadOnlySpan<byte> source) =>
        XPLMSetDatab(_handle, ref MemoryMarshal.GetReference(source), _offset, source.Length);
}

public sealed class Data<T> where T : unmanaged
{
    readonly nint _id;

    internal Data(nint id) => _id = id;

    public unsafe T Value
    {
        get
        {
            T value = default;
            var read = ByteVector.XPLMGetDatab(_id, ref *(byte*)&value, 0, sizeof(T));
            Debug.Assert(read == sizeof(T));
            return value;
        }
        set => ByteVector.XPLMSetDatab(_id, ref *(byte*)&value, 0, sizeof(T));
    }
}

public sealed partial class DataRef
{
    [LibraryImport(Defs.Lib)]
    private static partial nint XPLMFindDataRef(in byte name);

    public static DataRef? Find(ReadOnlySpan<byte> name) =>
        XPLMFindDataRef(in MemoryMarshal.AsRef<byte>(name)) is not 0 and var i ? new(i) : null;

    public static DataRef? Find(string name) => Find(Encoding.ASCII.GetBytes(name));

    internal readonly nint _id;

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMCanWriteDataRef(nint handle);

    public bool CanWrite => XPLMCanWriteDataRef(_id) != 0;

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMIsDataRefGood(nint handle);

    public bool IsGood => XPLMIsDataRefGood(_id) != 0;

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMGetDataRefTypes(nint handle);

    public DataTypes Types => (DataTypes)XPLMGetDataRefTypes(_id);

    [LibraryImport(Defs.Lib)]
    private static partial int XPLMGetDatai(nint handle);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSetDatai(nint handle, int value);

    public int AsInt
    {
        get => XPLMGetDatai(_id);
        set => XPLMSetDatai(_id, value);
    }

    [LibraryImport(Defs.Lib)]
    private static partial float XPLMGetDataf(nint handle);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSetDataf(nint handle, float value);

    public float AsFloat
    {
        get => XPLMGetDataf(_id);
        set => XPLMSetDataf(_id, value);
    }

    [LibraryImport(Defs.Lib)]
    private static partial double XPLMGetDatad(nint handle);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSetDatad(nint handle, double value);

    public double AsDouble
    {
        get => XPLMGetDatad(_id);
        set => XPLMSetDatad(_id, value);
    }

    public IntVector AsIntVector(int offset = 0) => new(_id, offset);

    public FloatVector AsFloatVector(int offset = 0) => new(_id, offset);

    public ByteVector AsByteVector(int offset = 0) => new(_id, offset);

    public Data<T> As<T>() where T : unmanaged => new(_id);

    public string AsString
    {
        get
        {
            Span<byte> destination = stackalloc byte[ByteVector.XPLMGetDatab(_id, ref Unsafe.NullRef<byte>(), 0, 0)];
            var read = ByteVector.XPLMGetDatab(_id, ref MemoryMarshal.GetReference(destination), 0, destination.Length);
            Debug.Assert(read == destination.Length);
            return Encoding.UTF8.GetString(destination);
        }
        set
        {
            var source = Encoding.UTF8.GetBytes(value);
            ByteVector.XPLMSetDatab(_id, ref MemoryMarshal.GetReference(source.AsSpan()), 0, source.Length);
        }
    }

    internal DataRef(nint id) => _id = id;
}

public interface IAccessor
{
    int AsInt { get => default; set { } }

    float AsFloat { get => default; set { } }

    double AsDouble { get => default; set { } }

    int CountIntVector() => default;
    int ReadIntVector(int offset, Span<int> destination) => default;
    void WriteIntVector(int offset, ReadOnlySpan<int> source) { }

    int CountFloatVector() => default;
    int ReadFloatVector(int offset, Span<float> destination) => default;
    void WriteFloatVector(int offset, ReadOnlySpan<float> source) { }

    int CountByteVector() => default;
    int ReadByteVector(int offset, Span<byte> destination) => default;
    void WriteByteVector(int offset, ReadOnlySpan<byte> source) { }
}

public sealed class IntAccessor : IAccessor
{
    public int AsInt { get; set; }

    public float AsFloat { get => AsInt; set => AsInt = (int)value; }

    public double AsDouble { get => AsInt; set => AsInt = (int)value; }
}

public sealed class FloatAccessor : IAccessor
{
    public float AsFloat { get; set; }

    public double AsDouble { get => AsFloat; set => AsFloat = (float)value; }
}

public sealed class DoubleAccessor : IAccessor
{
    public float AsFloat { get => (float)AsDouble; set => AsDouble = value; }

    public double AsDouble { get; set; }
}

public sealed class IntArrayAccessor : IAccessor
{
    int[] _array = [];

    public int[] IntArray
    {
        get => _array;
        set => _array = value ?? [];
    }

    public int CountIntVector() => _array.Length;
    public int ReadIntVector(int offset, Span<int> destination)
    {
        var length = Math.Min(_array.Length - offset, destination.Length);
        _array.AsSpan(offset, length).CopyTo(destination);
        return length;
    }
    public void WriteIntVector(int offset, ReadOnlySpan<int> source)
    {
        var count = Math.Min(_array.Length - offset, source.Length);
        source[..count].CopyTo(_array.AsSpan(offset, count));
    }
}

public sealed class FloatArrayAccessor : IAccessor
{
    float[] _array = [];

    public float[] FloatArray
    {
        get => _array;
        set => _array = value ?? [];
    }

    public int CountFloatVector() => _array.Length;
    public int ReadFloatVector(int offset, Span<float> destination)
    {
        var length = Math.Min(_array.Length - offset, destination.Length);
        _array.AsSpan(offset, length).CopyTo(destination);
        return length;
    }
    public void WriteFloatVector(int offset, ReadOnlySpan<float> source)
    {
        var count = Math.Min(_array.Length - offset, source.Length);
        source[..count].CopyTo(_array.AsSpan(offset, count));
    }
}

public sealed class DataAccessor : IAccessor
{
    byte[] _array = [];

    public byte[] Data
    {
        get => _array;
        set => _array = value ?? [];
    }

    public int CountByteVector() => _array.Length;
    public int ReadByteVector(int offset, Span<byte> destination)
    {
        var length = Math.Min(_array.Length - offset, destination.Length);
        _array.AsSpan(offset, length).CopyTo(destination);
        return length;
    }
    public void WriteByteVector(int offset, ReadOnlySpan<byte> source)
    {
        var count = Math.Min(_array.Length - offset, source.Length);
        source[..count].CopyTo(_array.AsSpan(offset, count));
    }
}

public sealed class Accessor<T> : IAccessor where T : unmanaged
{
    T _data;

    public ref T Data => ref _data;

    public Accessor() =>
        Debug.Assert(typeof(T) != typeof(int) && typeof(T) != typeof(float) && typeof(T) != typeof(double));

    public int CountByteVector() => Unsafe.SizeOf<T>();
    public unsafe int ReadByteVector(int offset, Span<byte> destination)
    {
        var length = Math.Min(sizeof(T) - offset, destination.Length);
        fixed (void* p = &_data)
            new ReadOnlySpan<byte>((byte*)p + offset, length).CopyTo(destination);
        return length;
    }
    public unsafe void WriteByteVector(int offset, ReadOnlySpan<byte> source)
    {
        var count = Math.Min(sizeof(T) - offset, source.Length);
        fixed (void* p = &_data)
            source[..count].CopyTo(new Span<byte>((byte*)p + offset, count));
    }
}

public sealed partial class DataRefRegistration : IDisposable
{
    public static DataRefRegistration Register<T>(ReadOnlySpan<byte> name, bool isWritable, Accessor<T> accessor) where T : unmanaged =>
        new(name, DataTypes.Data, isWritable, accessor);

    public static DataRefRegistration Register<T>(string name, bool isWritable, Accessor<T> accessor) where T : unmanaged =>
        new(Encoding.ASCII.GetBytes(name), DataTypes.Data, isWritable, accessor);

    public DataRef DataRef { get; }

    readonly GCHandle _handle;

    public DataRefRegistration(ReadOnlySpan<byte> name, bool isWritable, IntAccessor accessor) :
        this(name, DataTypes.Int | DataTypes.Float | DataTypes.Double, isWritable, accessor)
    { }

    public DataRefRegistration(ReadOnlySpan<byte> name, bool isWritable, FloatAccessor accessor) :
        this(name, DataTypes.Float | DataTypes.Double, isWritable, accessor)
    { }

    public DataRefRegistration(ReadOnlySpan<byte> name, bool isWritable, DoubleAccessor accessor) :
        this(name, DataTypes.Float | DataTypes.Double, isWritable, accessor)
    { }

    public DataRefRegistration(ReadOnlySpan<byte> name, bool isWritable, IntArrayAccessor accessor) :
       this(name, DataTypes.IntArray, isWritable, accessor)
    { }

    public DataRefRegistration(ReadOnlySpan<byte> name, bool isWritable, FloatArrayAccessor accessor) :
        this(name, DataTypes.FloatArray, isWritable, accessor)
    { }

    public DataRefRegistration(ReadOnlySpan<byte> name, bool isWritable, DataAccessor accessor) :
        this(name, DataTypes.Data, isWritable, accessor)
    { }

    [LibraryImport(Defs.Lib)]
    private unsafe static partial nint XPLMRegisterDataAccessor(ReadOnlySpan<byte> dataName, DataTypes dataType, int isWritable,
        delegate* unmanaged<nint, int> readInt, delegate* unmanaged<nint, int, void> writeInt,
        delegate* unmanaged<nint, float> readFloat, delegate* unmanaged<nint, float, void> writeFloat,
        delegate* unmanaged<nint, double> readDouble, delegate* unmanaged<nint, double, void> writeDouble,
        delegate* unmanaged<nint, int*, int, int, int> readIntVector,
        delegate* unmanaged<nint, int*, int, int, void> writeIntVector,
        delegate* unmanaged<nint, float*, int, int, int> readFloatVector,
        delegate* unmanaged<nint, float*, int, int, void> writeFloatVector,
        delegate* unmanaged<nint, byte*, int, int, int> readByteVector,
        delegate* unmanaged<nint, byte*, int, int, void> writeByteVector,
        nint readRefcon, nint writeRefcon);

    public unsafe DataRefRegistration(ReadOnlySpan<byte> name, DataTypes type, bool isWritable, IAccessor accessor)
    {
        static IAccessor A(nint handle) => (IAccessor)GCHandle.FromIntPtr(handle).Target!;

        [UnmanagedCallersOnly]
        static int ReadInt(nint handle) => A(handle).AsInt;

        [UnmanagedCallersOnly]
        static void WriteInt(nint handle, int value) => A(handle).AsInt = value;

        [UnmanagedCallersOnly]
        static float ReadFloat(nint handle) => A(handle).AsFloat;

        [UnmanagedCallersOnly]
        static void WriteFloat(nint handle, float value) => A(handle).AsFloat = value;

        [UnmanagedCallersOnly]
        static double ReadDouble(nint handle) => A(handle).AsDouble;

        [UnmanagedCallersOnly]
        static void WriteDouble(nint handle, double value) => A(handle).AsDouble = value;

        [UnmanagedCallersOnly]
        static int ReadIntVector(nint handle, int* values, int offset, int maxLength)
        {
            var a = A(handle);
            return values == null ? a.CountIntVector() : a.ReadIntVector(offset, new(values, maxLength));
        }

        [UnmanagedCallersOnly]
        static void WriteIntVector(nint handle, int* values, int offset, int count) =>
            A(handle).WriteIntVector(offset, new(values, count));

        [UnmanagedCallersOnly]
        static int ReadFloatVector(nint handle, float* values, int offset, int maxLength)
        {
            var a = A(handle);
            return values == null ? a.CountFloatVector() : a.ReadFloatVector(offset, new(values, maxLength));
        }

        [UnmanagedCallersOnly]
        static void WriteFloatVector(nint handle, float* values, int offset, int count) =>
            A(handle).WriteFloatVector(offset, new(values, count));

        [UnmanagedCallersOnly]
        static int ReadByteVector(nint handle, byte* values, int offset, int maxLength)
        {
            var a = A(handle);
            return values == null ? a.CountByteVector() : a.ReadByteVector(offset, new(values, maxLength));
        }

        [UnmanagedCallersOnly]
        static void WriteByteVector(nint handle, byte* values, int offset, int count) =>
            A(handle).WriteByteVector(offset, new(values, count));

        var isInt = type.HasFlag(DataTypes.Int);
        var isFloat = type.HasFlag(DataTypes.Float);
        var isDouble = type.HasFlag(DataTypes.Double);
        var isIntArray = type.HasFlag(DataTypes.IntArray);
        var isFloatArray = type.HasFlag(DataTypes.FloatArray);
        var isData = type.HasFlag(DataTypes.Data);
        nint r = GCHandle.ToIntPtr(_handle = GCHandle.Alloc(accessor));
        DataRef = new(XPLMRegisterDataAccessor(name, type, isWritable ? 1 : 0,
            isInt ? &ReadInt : null, isWritable && isInt ? &WriteInt : null,
            isFloat ? &ReadFloat : null, isWritable && isFloat ? &WriteFloat : null,
            isDouble ? &ReadDouble : null, isWritable && isDouble ? &WriteDouble : null,
            isIntArray ? &ReadIntVector : null, isWritable && isIntArray ? &WriteIntVector : null,
            isFloatArray ? &ReadFloatVector : null, isWritable && isFloatArray ? &WriteFloatVector : null,
            isData ? &ReadByteVector : null, isWritable && isData ? &WriteByteVector : null,
            r, isWritable ? r : default
        ));
    }

    public DataRefRegistration(string name, bool isWritable, IntAccessor accessor) :
        this(Encoding.ASCII.GetBytes(name), isWritable, accessor)
    { }

    public DataRefRegistration(string name, bool isWritable, FloatAccessor accessor) :
        this(Encoding.ASCII.GetBytes(name), isWritable, accessor)
    { }

    public DataRefRegistration(string name, bool isWritable, DoubleAccessor accessor) :
        this(Encoding.ASCII.GetBytes(name), isWritable, accessor)
    { }

    public DataRefRegistration(string name, bool isWritable, IntArrayAccessor accessor) :
       this(Encoding.ASCII.GetBytes(name), isWritable, accessor)
    { }

    public DataRefRegistration(string name, bool isWritable, FloatArrayAccessor accessor) :
        this(Encoding.ASCII.GetBytes(name), isWritable, accessor)
    { }

    public DataRefRegistration(string name, bool isWritable, DataAccessor accessor) :
        this(Encoding.ASCII.GetBytes(name), isWritable, accessor)
    { }

    public unsafe DataRefRegistration(string name, DataTypes type, bool isWritable, IAccessor accessor) :
        this(Encoding.ASCII.GetBytes(name), type, isWritable, accessor)
    { }

    ~DataRefRegistration() => Dispose();

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMUnregisterDataAccessor(nint dataRef);

    bool _disposed;
    public void Dispose()
    {
        if (!_disposed)
        {
            XPLMUnregisterDataAccessor(DataRef._id);
            _handle.Free();
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }
}

public sealed partial class Shared : IDisposable
{
    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial int XPLMShareData(string dataName, DataTypes dataType, delegate* unmanaged<nint, void> notification, nint handle);

    public static unsafe bool TryShare(string name, DataTypes type, Action notification, [MaybeNullWhen(false)] out Shared? share)
    {
        var h = GCHandle.Alloc(notification);
        if (XPLMShareData(name, type, &Notify, GCHandle.ToIntPtr(h)) == 0)
        {
            share = null;
            return false;
        }
        share = new(name, type, h);
        return true;
    }

    [UnmanagedCallersOnly]
    internal static void Notify(nint handle) => ((Action)GCHandle.FromIntPtr(handle).Target!)();

    public string Name { get; }

    public DataTypes Type { get; }

    readonly GCHandle _handle;

    internal Shared(string name, DataTypes type, GCHandle handle) =>
        (Name, Type, _handle) = (name, type, handle);

    ~Shared() => Dispose();

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static unsafe partial void XPLMUnshareData(string dataName, DataTypes dataType, delegate* unmanaged<nint, void> notification, nint handle);

    bool _disposed;
    public unsafe void Dispose()
    {
        if (!_disposed)
        {
            XPLMUnshareData(Name, Type, &Notify, GCHandle.ToIntPtr(_handle));
            _handle.Free();
            _disposed = true;
        }
        GC.SuppressFinalize(this);
    }

    public DataRef? FindDataRef() => DataRef.Find(Name);
}