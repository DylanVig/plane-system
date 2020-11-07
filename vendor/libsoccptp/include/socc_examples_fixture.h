/* Copyright 2015 Sony Corporation */
/* Sony Confidential               */

#ifndef __SOCC_EXAMPLES_FIXTURE_H__
#define __SOCC_EXAMPLES_FIXTURE_H__

#include <time.h>
#include "rust/cxx.h"

#include <memory>
#include <string>
#include <string.h>
#include <socc_ptp.h>

#include "socc_examples_log.h"
#include "parser.h"

using namespace com::sony::imaging::remote;

typedef struct _ObjectInfo_t
{
    uint32_t StorageId;
    uint16_t ObjectFormat;
    uint16_t ProtectionStatus;
    uint32_t ObjectCompressedSize;
} ObjectInfo_t;

typedef struct _LiveViewInfo_t
{
    uint32_t Offset_to_LiveView_Image;
    uint32_t LiveVew_Image_Size;
} LiveViewInfo_t;

class socc_examples_ptpstring
{
public:
    socc_examples_ptpstring(const char *c, size_t length)
    {
        std::string ascii(c, length);
        bytes_size = 1 + (ascii.size() + 1) * 2;
        bytes = new uint8_t[bytes_size];
        bytes[0] = ascii.size();
        for (int i = 0; i < ascii.size() + 1; i++)
        {
            bytes[1 + i * 2] = c[i];
            bytes[1 + i * 2 + 1] = 0;
        }
    }

    socc_examples_ptpstring(const char *c)
    {
        std::string ascii(c);
        bytes_size = 1 + (ascii.size() + 1) * 2;
        bytes = new uint8_t[bytes_size];
        bytes[0] = ascii.size();
        for (int i = 0; i < ascii.size() + 1; i++)
        {
            bytes[1 + i * 2] = c[i];
            bytes[1 + i * 2 + 1] = 0;
        }
    }
    ~socc_examples_ptpstring()
    {
        delete[] bytes;
    }
    uint8_t *bytes;
    uint16_t bytes_size;
};

class socc_examples_fixture
{
public:
    socc_examples_fixture(socc_ptp &ptp) : ptp(ptp)
    {
    }

    /* connect */
    int connect()
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        return ptp.connect();
    }

    /* disconnect */
    int disconnect()
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        return ptp.disconnect();
    }

    /* OpenSession */
    int OpenSession(uint32_t session_id = 1)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        uint32_t params[1];
        Container response;
        params[0] = session_id;
        ret = ptp.send(0x1002, params, 1, response, NULL, 0);

        log.assert_socc("rc", (uint16_t)0x2001, response.code);
        return ret;
    }
    /* CloseSession */
    void CloseSession()
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        Container response;
        ret = ptp.send(0x1003, NULL, 0, response, NULL, 0);

        log.assert_socc("ret", 0, ret);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);
        fprintf(stderr, "\x1b[31mPower off the camera or disconnect USB cable before next operations.\n\x1b[39m");
    }

    /* GetObjectInfo */
    void GetObjectInfo(uint32_t handle, ObjectInfo_t *object_info = NULL)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        ObjectInfo_t *data = NULL;
        uint32_t size = 0;
        uint32_t params[1];
        Container response;
        params[0] = handle;
        ret = ptp.receive(0x1008, params, 1, response, (void **)&data, size);

        log.assert_socc("ret", 0, ret);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);

        if (object_info != NULL)
        {
            *object_info = *data;
        }

        ptp.dispose_data((void **)&data);
    }

    /* GetObject */
    void GetObject(uint32_t handle, void **object_data = NULL, uint32_t *compressed_size = NULL)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        void *data = NULL;
        FILE *fpo = NULL;
        uint32_t size = 0;
        uint32_t params[1];
        Container response;
        params[0] = handle;

        ret = ptp.receive(0x1009, params, 1, response, (void **)&data, size);

        log.assert_socc("ret", 0, ret);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);

        if (object_data != NULL && compressed_size != NULL)
        {
            *compressed_size = size;
            *object_data = malloc(size);
            memcpy(*object_data, data, size);
        }
        ptp.dispose_data((void **)&data);
    }

    /* SDIO_GetAllExtDevicePropInfo */
    int SDIO_GetAllExtDevicePropInfo(SDIDevicePropInfoDatasetArray **array)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        void *data = NULL;
        uint32_t size = 0;
        uint32_t params[0];
        Container response;

        ret = ptp.receive(0x96F6, params, 0, response, (void **)&data, size);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);

        *array = new SDIDevicePropInfoDatasetArray(data);
        ptp.dispose_data((void **)&data);
        return ret;
    }

    template <typename T>
    SDIDevicePropInfoDataset* wait_for_IsEnable(uint16_t code, T expect, int count = 1000)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        SDIDevicePropInfoDataset *dataset = NULL;
        while (count > 0)
        {
            count--;
            SDIDevicePropInfoDatasetArray *array = NULL;
            int ret = SDIO_GetAllExtDevicePropInfo(&array);
            log.assert_socc("ret", 0, ret);

            if (array == NULL)
            {
                continue;
            }
            dataset = array->get(code);
            if (dataset == NULL)
            {
                delete array;
                continue;
            }
            T isEnable = dataset->IsEnable;
            delete array;
            if (isEnable == expect)
            {
                break;
            }
        }
        if (dataset == NULL)
        {
            log.w("SDIDevicePropInfoDataset,property %04x,not found", code);
        }
        return dataset;
    }

    template <typename T>
    size_t wait_for_IsEnable_usize(uint16_t code, T expect, int count = 1000)
    {
        return size_t(wait_for_IsEnable(code, expect, count));
    }

    template <typename T>
    T *get_CurrentValue(uint16_t code, int count = 1000)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        T *CurrentValue = NULL;
        DataTypeInteger<T> *dataset = NULL;
        while (count > 0)
        {
            count--;
            SDIDevicePropInfoDatasetArray *array = NULL;
            SDIO_GetAllExtDevicePropInfo(&array);
            if (array == NULL)
            {
                continue;
            }
            dataset = (DataTypeInteger<T> *)array->get(code);
            if (dataset == NULL)
            {
                delete array;
                continue;
            }
            CurrentValue = new T();
            *CurrentValue = dataset->CurrentValue;
            delete array;
            break;
        }
        return CurrentValue;
    }

    template <typename T>
    int wait_for_CurrentValue(uint16_t code, T expect, int count = 1000)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        DataTypeInteger<T> *dataset = NULL;
        while (count > 0)
        {
            count--;
            SDIDevicePropInfoDatasetArray *array = NULL;
            int ret = SDIO_GetAllExtDevicePropInfo(&array);

            if (ret < 0)
                return ret;

            if (array == NULL)
            {
                continue;
            }

            dataset = (DataTypeInteger<T> *)array->get(code);
            if (dataset == NULL)
            {
                delete array;
                continue;
            }

            T CurrentValue = dataset->CurrentValue;
            delete array;
            if (CurrentValue == expect)
            {
                break;
            }
        }

        if (dataset == NULL)
        {
            log.w("SDIDevicePropInfoDataset,property %04x,not found", code);
        }

        return 0;
    }

    template <typename SDIControl_value_t>
    int SDIO_ControlDevice(uint16_t code, SDIControl_value_t value)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        uint32_t params[1];
        Container response;
        params[0] = code;
        ret = ptp.send(0x96F8, params, 1, response, &value, sizeof(value));
        log.assert_socc("rc", (uint16_t)0x2001, response.code);
        return ret;
    }

    template <typename T>
    int SDIO_SetExtDevicePropValue(uint16_t code, T value)
    {
        socc_examples_log log("SDIO_SetExtDevicePropValue", socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        int ret;
        uint32_t params[1];
        Container response;
        params[0] = code;
        ret = ptp.send(0x96FA, params, 1, response, &value, sizeof(value));
        log.assert_socc("rc", (uint16_t)0x2001, response.code);
        return ret;
    }

    int SDIO_SetExtDevicePropValue(uint16_t code, socc_examples_ptpstring ptpstring)
    {
        socc_examples_log log("SDIO_SetExtDevicePropValue", socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        int ret;
        uint32_t params[1];
        Container response;
        params[0] = code;
        ret = ptp.send(0x96FA, params, 1, response, ptpstring.bytes, ptpstring.bytes_size);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);
        return ret;
    }

    int SDIO_SetExtDevicePropValue_str(uint16_t code, rust::Str str)
    {
        return SDIO_SetExtDevicePropValue(code, socc_examples_ptpstring(str.data(), str.length()));
    }

    /* SDIO_GetExtDeviceInfo */
    int SDIO_GetExtDeviceInfo(uint16_t initiator_version = 0x00C8, uint16_t *actual_initiator_version = NULL)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        bool escape = false;
        int ret;
        uint16_t *version;
        uint32_t size = 0;
        uint32_t params[1];
        Container response;
        params[0] = (uint32_t)initiator_version;

        ret = ptp.receive(0x96FD, params, 1, response, (void **)&version, size);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);

        if (actual_initiator_version != NULL)
        {
            *actual_initiator_version = *version;
        }
        ptp.dispose_data((void **)&version);
        return ret;
    }

    int wait_for_InitiatorVersion(uint16_t expect = 0x00C8, int retry_count = 1000)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);
        uint16_t actual;
        while (retry_count > 0)
        {
            actual = ~expect;
            retry_count--;
            int ret = SDIO_GetExtDeviceInfo((uint32_t)expect, &actual);
            if (ret < 0)
            {
                return ret;
            }
            if (expect == actual)
            {
                break;
            }
        }
        log.assert_socc("InitiatorVersion", (uint16_t)expect, actual);
        return 0;
    }

    /* SDIO_Connect */
    int SDIO_Connect(uint32_t phase_type, uint32_t keycode1 = 0x0000DA01, uint32_t keycode2 = 0x0000DA01)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        uint64_t *data;
        uint32_t params[3];
        Container response;
        uint32_t size;
        params[0] = phase_type;
        params[1] = keycode1;
        params[2] = keycode2;
        ret = ptp.receive(0x96FE, params, 3, response, (void **)&data, size);
        log.assert_socc("rc", (uint16_t)0x2001, response.code);

        ptp.dispose_data((void **)&data);
        return ret;
    }

    void wait_event(uint16_t code)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        bool escape = false;
        while (escape == false)
        {
            Container event;
            log.i("wait start %x", code);
            ret = ptp.wait_event(event);
            if (ret == SOCC_ERROR_USB_TIMEOUT)
            {
                log.i("timeout");
                continue;
            }
            if (event.code == code)
            {
                log.i("EventCode:%x EventParam1:%x", event.code, event.param1);
                break;
            }
            log.assert_socc("ret", 0, ret);
        };
    }

    void drop_event(uint16_t code)
    {
        socc_examples_log log(__FUNCTION__, socc_examples_log::SOCC_EXAMPLES_LOG_INFO);

        int ret;
        Container event;
        log.i("drop_event %x", code);
        ret = ptp.wait_event(event);
        if (ret == SOCC_ERROR_USB_TIMEOUT)
        {
            log.i("timeout");
        }
        if (event.code == code)
        {
            log.i("EventCode:%x EventParam1:%x", event.code, event.param1);
        }
        log.i("ret:%x", ret);
    }

    void milisleep(uint16_t msec)
    {
        struct timespec req;
        req.tv_nsec = (msec * 1000 * 1000) % 1000000000;
        req.tv_sec = msec / 1000;
        nanosleep(&req, NULL);
    }

private:
    com::sony::imaging::remote::socc_ptp &ptp;
};

std::unique_ptr<socc_ptp> make_ptp()
{
    return std::make_unique<socc_ptp>(0, 0);
}

std::unique_ptr<socc_examples_fixture> make_fixture(socc_ptp &ptp)
{
    return std::make_unique<socc_examples_fixture>(ptp);
}

#endif
