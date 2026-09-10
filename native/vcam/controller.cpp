// SPDX-License-Identifier: GPL-3.0-or-later
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>
#include <mfvirtualcamera.h>
#include <wrl/client.h>
#include <iostream>
#include <string>
#include <vector>
#include <stdexcept>
#include <iomanip>
using Microsoft::WRL::ComPtr;
constexpr wchar_t ClassId[] = L"{EAD49F23-8F8C-47E7-A78C-F7744A6D54C9}";
constexpr wchar_t CameraName[] = L"Open EOS Camera (test pattern)";

void check(HRESULT hr, const char* operation) {
    if (FAILED(hr)) {
        std::cerr << operation << " failed: 0x" << std::hex << static_cast<unsigned long>(hr) << '\n';
        throw std::runtime_error(operation);
    }
}

void validate(IMFMediaSource* source, DWORD format) {
    ComPtr<IMFSourceReader> reader;
    check(MFCreateSourceReaderFromMediaSource(source, nullptr, &reader), "Create source reader");
    ComPtr<IMFMediaType> type;
    check(reader->GetNativeMediaType(0, format, &type), "Get format");
    check(reader->SetCurrentMediaType(0, nullptr, type.Get()), "Select format");
    UINT32 width = 0, height = 0;
    check(MFGetAttributeSize(type.Get(), MF_MT_FRAME_SIZE, &width, &height), "Get dimensions");
    GUID subtype;
    check(type->GetGUID(MF_MT_SUBTYPE, &subtype), "Get pixel format");
    if (width != (format < 2 ? 1280u : 720u) || height != (format < 2 ? 720u : 1280u)
        || subtype != (format % 2 == 0 ? MFVideoFormat_NV12 : MFVideoFormat_RGB32))
        throw std::runtime_error("Native format does not match its advertised profile");
    check(reader->SetStreamSelection(0, TRUE), "Select stream");
    LONGLONG previous = -1;
    unsigned received = 0, changes = 0;
    std::vector<BYTE> prior;
    for (unsigned request = 0; request < 120 && received < 30; ++request) {
        DWORD flags = 0;
        LONGLONG timestamp = 0;
        ComPtr<IMFSample> sample;
        check(reader->ReadSample(0, 0, nullptr, &flags, &timestamp, &sample), "Read frame");
        if (flags & (MF_SOURCE_READERF_ERROR | MF_SOURCE_READERF_ENDOFSTREAM)) throw std::runtime_error("Stream stopped unexpectedly");
        if (!sample) continue;
        if (timestamp <= previous) throw std::runtime_error("Frame timestamps did not advance");
        previous = timestamp;
        ComPtr<IMFMediaBuffer> buffer;
        check(sample->ConvertToContiguousBuffer(&buffer), "Get pixels");
        BYTE* data = nullptr;
        DWORD length = 0;
        check(buffer->Lock(&data, nullptr, &length), "Lock pixels");
        std::vector<BYTE> pixels(data, data + length);
        buffer->Unlock();
        const size_t minimumSize = subtype == MFVideoFormat_NV12 ? size_t(width) * height * 3 / 2 : size_t(width) * height * 4;
        if (pixels.size() < minimumSize) throw std::runtime_error("Truncated frame");
        if (!prior.empty() && pixels != prior) ++changes;
        prior = std::move(pixels);
        ++received;
        Sleep(34);
    }
    check(reader->Flush(0), "Flush reader");
    reader.Reset();
    if (received != 30 || changes < 10) throw std::runtime_error("Expected 30 moving frames");
    std::cout << "PASS: " << received << " frames, " << width << "x" << height
              << ", " << changes << " pixel changes, increasing timestamps.\n";
}

int wmain(int argc, wchar_t** argv) {
    if (argc < 2) {
        std::cout << "eos-vcam self-test <source.dll> | run | remove\n";
        return 2;
    }
    HRESULT com = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    if (FAILED(com)) return 1;
    int result = 0;
    HMODULE module = nullptr;
    try {
        check(MFStartup(MF_VERSION), "Start Media Foundation");
        const std::wstring action = argv[1];
        if (action == L"self-test" && argc == 3) {
            module = LoadLibraryExW(argv[2], nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
            if (!module) throw std::runtime_error("Could not load camera DLL; use its absolute path");
            using FactoryFunction = HRESULT(WINAPI*)(REFCLSID, REFIID, void**);
            auto getFactory = reinterpret_cast<FactoryFunction>(GetProcAddress(module, "DllGetClassObject"));
            if (!getFactory) throw std::runtime_error("Camera DLL has no class factory");
            CLSID clsid;
            check(CLSIDFromString(ClassId, &clsid), "Parse class ID");
            ComPtr<IClassFactory> factory;
            check(getFactory(clsid, IID_PPV_ARGS(&factory)), "Get class factory");
            ComPtr<IMFActivate> activate;
            check(factory->CreateInstance(nullptr, IID_PPV_ARGS(&activate)), "Create activation");
            for (DWORD format = 0; format < 4; ++format) {
                ComPtr<IMFMediaSource> source;
                check(activate->ActivateObject(IID_PPV_ARGS(&source)), "Activate camera source");
                try { validate(source.Get(), format); }
                catch (...) { source->Shutdown(); activate->DetachObject(); throw; }
                check(source->Shutdown(), "Shutdown source");
                activate->DetachObject();
            }
        } else if (action == L"run" || action == L"remove") {
            ComPtr<IMFVirtualCamera> camera;
            check(MFCreateVirtualCamera(MFVirtualCameraType_SoftwareCameraSource,
                MFVirtualCameraLifetime_Session, MFVirtualCameraAccess_CurrentUser,
                CameraName, ClassId, nullptr, 0, &camera), "Create virtual camera");
            if (action == L"remove") {
                check(camera->Remove(), "Remove virtual camera");
            } else {
                check(camera->Start(nullptr), "Start virtual camera (register the DLL first)");
                std::wcout << CameraName << L" is available. This is an animated TEST PATTERN.\n"
                    << L"Select it in a camera app. Press Enter here to stop and remove it.\n";
                std::wstring line;
                std::getline(std::wcin, line);
                check(camera->Stop(), "Stop virtual camera");
                check(camera->Remove(), "Remove virtual camera");
            }
        } else { throw std::runtime_error("Unknown command or wrong arguments"); }
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << '\n';
        result = 1;
    }
    MFShutdown();
    // MF may retain asynchronous work until shutdown has completed. The operating
    // system releases the loaded DLL on process exit; never unload it prematurely.
    CoUninitialize();
    return result;
}
