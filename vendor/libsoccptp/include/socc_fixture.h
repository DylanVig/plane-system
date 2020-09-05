/* Copyright 2015 Sony Corporation */
/* Sony Confidential               */

#ifndef __SOCC_EXAMPLES_FIXTURE_H__
#define __SOCC_EXAMPLES_FIXTURE_H__

#include <time.h>

#include <string.h>
#include <socc_ptp.h>

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
    socc_examples_fixture(com::sony::imaging::remote::socc_ptp &ptp) : ptp(ptp)
    {
    }

    /* connect */
    void connect()
    {
        int ret = ptp.connect();
    }

    /* disconnect */
    void disconnect()
    {
        int ret = ptp.disconnect();
    }

    /* OpenSession */
    void OpenSession(uint32_t session_id = 1)
    {

        int ret;
        uint32_t params[1];
        Container response;
        params[0] = session_id;
        ret = ptp.send(0x1002, params, 1, response, NULL, 0);
    }
    /* CloseSession */
    void CloseSession()
    {

        int ret;
        Container response;
        ret = ptp.send(0x1003, NULL, 0, response, NULL, 0);
        fprintf(stderr, "\x1b[31mPower off the camera or disconnect USB cable before next operations.\n\x1b[39m");
    }

    /* GetObjectInfo */
    void GetObjectInfo(uint32_t handle, ObjectInfo_t *object_info = NULL)
    {
        int ret;
        ObjectInfo_t *data = NULL;
        uint32_t size = 0;
        uint32_t params[1];
        Container response;
        params[0] = handle;
        ret = ptp.receive(0x1008, params, 1, response, (void **)&data, size);

        if (object_info != NULL)
        {
            *object_info = *data;
        }

        ptp.dispose_data((void **)&data);
    }

    /* GetObject */
    void GetObject(uint32_t handle, void **object_data = NULL, uint32_t *compressed_size = NULL)
    {
        int ret;
        void *data = NULL;
        FILE *fpo = NULL;
        uint32_t size = 0;
        uint32_t params[1];
        Container response;
        params[0] = handle;

        ret = ptp.receive(0x1009, params, 1, response, (void **)&data, size);

        if (object_data != NULL && compressed_size != NULL)
        {
            *compressed_size = size;
            *object_data = malloc(size);
            memcpy(*object_data, data, size);
        }
        ptp.dispose_data((void **)&data);
    }

    /* SDIO_GetAllExtDevicePropInfo */
    void SDIO_GetAllExtDevicePropInfo(SDIDevicePropInfoDatasetArray **array)
    {
        int ret;
        void *data = NULL;
        uint32_t size = 0;
        uint32_t params[0];
        Container response;

        ret = ptp.receive(0x96F6, params, 0, response, (void **)&data, size);

        *array = new SDIDevicePropInfoDatasetArray(data);
        ptp.dispose_data((void **)&data);
    }

    template <typename T>
    SDIDevicePropInfoDataset *wait_for_IsEnable(uint16_t code, T expect, int count = 1000)
    {
        SDIDevicePropInfoDataset *dataset = NULL;
        while (count > 0)
        {
            count--;
            SDIDevicePropInfoDatasetArray *array = NULL;
            SDIO_GetAllExtDevicePropInfo(&array);
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
        }
        return dataset;
    }

    template <typename T>
    T *get_CurrentValue(uint16_t code, int count = 1000)
    {
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
    void wait_for_CurrentValue(uint16_t code, T expect, int count = 1000)
    {
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
            T CurrentValue = dataset->CurrentValue;
            delete array;
            if (CurrentValue == expect)
            {
                break;
            }
        }
        if (dataset == NULL)
        {
        }
    }

    template <typename SDIControl_value_t>
    void SDIO_ControlDevice(uint16_t code, SDIControl_value_t value)
    {

        int ret;
        uint32_t params[1];
        Container response;
        params[0] = code;
        ret = ptp.send(0x96F8, params, 1, response, &value, sizeof(value));
    }

    template <typename T>
    void SDIO_SetExtDevicePropValue(uint16_t code, T value)
    {
        int ret;
        uint32_t params[1];
        Container response;
        params[0] = code;
        ret = ptp.send(0x96FA, params, 1, response, &value, sizeof(value));
    }

    void SDIO_SetExtDevicePropValue(uint16_t code, socc_examples_ptpstring ptpstring)
    {
        int ret;
        uint32_t params[1];
        Container response;
        params[0] = code;
        ret = ptp.send(0x96FA, params, 1, response, ptpstring.bytes, ptpstring.bytes_size);
    }

    /* SDIO_GetExtDeviceInfo */
    void SDIO_GetExtDeviceInfo(uint16_t initiator_version = 0x00C8, uint16_t *actual_initiator_version = NULL)
    {

        bool escape = false;
        int ret;
        uint16_t *version;
        uint32_t size = 0;
        uint32_t params[1];
        Container response;
        params[0] = (uint32_t)initiator_version;

        ret = ptp.receive(0x96FD, params, 1, response, (void **)&version, size);

        if (actual_initiator_version != NULL)
        {
            *actual_initiator_version = *version;
        }
        ptp.dispose_data((void **)&version);
    }
    void wait_for_InitiatorVersion(uint16_t expect = 0x00C8, int retry_count = 1000)
    {
        uint16_t actual;
        while (retry_count > 0)
        {
            actual = ~expect;
            retry_count--;
            SDIO_GetExtDeviceInfo((uint32_t)expect, &actual);
            if (expect == actual)
            {
                break;
            }
        }
    }

    /* SDIO_Connect */
    void SDIO_Connect(uint32_t phase_type, uint32_t keycode1 = 0x0000DA01, uint32_t keycode2 = 0x0000DA01)
    {

        int ret;
        uint64_t *data;
        uint32_t params[3];
        Container response;
        uint32_t size;
        params[0] = phase_type;
        params[1] = keycode1;
        params[2] = keycode2;
        ret = ptp.receive(0x96FE, params, 3, response, (void **)&data, size);

        ptp.dispose_data((void **)&data);
    }

    void wait_event(uint16_t code)
    {

        int ret;
        bool escape = false;
        while (escape == false)
        {
            Container event;
            ret = ptp.wait_event(event);
            if (ret == SOCC_ERROR_USB_TIMEOUT)
            {
                continue;
            }
            if (event.code == code)
            {
                break;
            }
        };
    }

    void drop_event(uint16_t code)
    {

        int ret;
        Container event;
        ret = ptp.wait_event(event);
        if (ret == SOCC_ERROR_USB_TIMEOUT)
        {
        }
        if (event.code == code)
        {
        }
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

#endif
