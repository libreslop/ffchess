function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_debug_string_5398f5bb970e0daa: function(arg0, arg1) {
            const ret = debugString(getObject(arg1));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_3c846841762788c1: function(arg0) {
            const ret = typeof(getObject(arg0)) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_string_7ef6b97b02428fae: function(arg0) {
            const ret = typeof(getObject(arg0)) === 'string';
            return ret;
        },
        __wbg___wbindgen_is_undefined_52709e72fb9f179c: function(arg0) {
            const ret = getObject(arg0) === undefined;
            return ret;
        },
        __wbg___wbindgen_number_get_34bb9d9dcfa21373: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_395e606bd0ee4427: function(arg0, arg1) {
            const obj = getObject(arg1);
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_6ddd609b62940d55: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_6b5b6b8576d35cb1: function(arg0) {
            getObject(arg0)._wbg_cb_unref();
        },
        __wbg_addColorStop_3bd77f997fb1fa1c: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            getObject(arg0).addColorStop(arg1, getStringFromWasm0(arg2, arg3));
        }, arguments); },
        __wbg_addEventListener_2d985aa8a656f6dc: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3));
        }, arguments); },
        __wbg_addEventListener_97281b0177d72360: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), getObject(arg4));
        }, arguments); },
        __wbg_arc_775d5170fd5e7a80: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            getObject(arg0).arc(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_beginPath_596efed55075dbc3: function(arg0) {
            getObject(arg0).beginPath();
        },
        __wbg_body_5eb99e7257e5ae34: function(arg0) {
            const ret = getObject(arg0).body;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_bubbles_b29cbc5a1ead8421: function(arg0) {
            const ret = getObject(arg0).bubbles;
            return ret;
        },
        __wbg_button_bdc91677bd7bbf58: function(arg0) {
            const ret = getObject(arg0).button;
            return ret;
        },
        __wbg_cache_key_21c521bc3dd66e9b: function(arg0) {
            const ret = getObject(arg0).__yew_subtree_cache_key;
            return isLikeNone(ret) ? 0x100000001 : (ret) >>> 0;
        },
        __wbg_cancelAnimationFrame_43fad84647f46036: function() { return handleError(function (arg0, arg1) {
            getObject(arg0).cancelAnimationFrame(arg1);
        }, arguments); },
        __wbg_cancelBubble_a023be4f28708976: function(arg0) {
            const ret = getObject(arg0).cancelBubble;
            return ret;
        },
        __wbg_changedTouches_3c03cd569b57245b: function(arg0) {
            const ret = getObject(arg0).changedTouches;
            return addHeapObject(ret);
        },
        __wbg_childNodes_2deab43783cba95d: function(arg0) {
            const ret = getObject(arg0).childNodes;
            return addHeapObject(ret);
        },
        __wbg_clearInterval_16e8cbbce92291d0: function(arg0) {
            const ret = clearInterval(takeObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_clearTimeout_113b1cde814ec762: function(arg0) {
            const ret = clearTimeout(takeObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_clientX_7db5d6e77c921464: function(arg0) {
            const ret = getObject(arg0).clientX;
            return ret;
        },
        __wbg_clientX_eff94e775c0667a2: function(arg0) {
            const ret = getObject(arg0).clientX;
            return ret;
        },
        __wbg_clientY_2cf964b439a5974f: function(arg0) {
            const ret = getObject(arg0).clientY;
            return ret;
        },
        __wbg_clientY_6293e127369957bf: function(arg0) {
            const ret = getObject(arg0).clientY;
            return ret;
        },
        __wbg_cloneNode_c2549c13a4ecec77: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).cloneNode();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_close_af26905c832a88cb: function() { return handleError(function (arg0) {
            getObject(arg0).close();
        }, arguments); },
        __wbg_closest_ee628ba349731f6d: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = getObject(arg0).closest(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments); },
        __wbg_code_aea376e2d265a64f: function(arg0) {
            const ret = getObject(arg0).code;
            return ret;
        },
        __wbg_complete_6f48c2606490357b: function(arg0) {
            const ret = getObject(arg0).complete;
            return ret;
        },
        __wbg_composedPath_bb47f74544516629: function(arg0) {
            const ret = getObject(arg0).composedPath();
            return addHeapObject(ret);
        },
        __wbg_createElementNS_aac38e987dc3e061: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            const ret = getObject(arg0).createElementNS(arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_createElement_9b0aab265c549ded: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = getObject(arg0).createElement(getStringFromWasm0(arg1, arg2));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_createRadialGradient_5d814c3de73f7596: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            const ret = getObject(arg0).createRadialGradient(arg1, arg2, arg3, arg4, arg5, arg6);
            return addHeapObject(ret);
        }, arguments); },
        __wbg_createTextNode_1997fd99f09c6afd: function(arg0, arg1, arg2) {
            const ret = getObject(arg0).createTextNode(getStringFromWasm0(arg1, arg2));
            return addHeapObject(ret);
        },
        __wbg_data_a3d9ff9cdd801002: function(arg0) {
            const ret = getObject(arg0).data;
            return addHeapObject(ret);
        },
        __wbg_deltaY_c6ccae416e166d01: function(arg0) {
            const ret = getObject(arg0).deltaY;
            return ret;
        },
        __wbg_dispatchEvent_29145a50abb697bc: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg0).dispatchEvent(getObject(arg1));
            return ret;
        }, arguments); },
        __wbg_documentElement_bef4a2d2289074bb: function(arg0) {
            const ret = getObject(arg0).documentElement;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_document_c0320cd4183c6d9b: function(arg0) {
            const ret = getObject(arg0).document;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_drawImage_09a18e68469e9b9b: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            getObject(arg0).drawImage(getObject(arg1), arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_drawImage_ce7fb4f15446013d: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            getObject(arg0).drawImage(getObject(arg1), arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbg_encodeURIComponent_92643eb91e22a715: function(arg0, arg1) {
            const ret = encodeURIComponent(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        },
        __wbg_error_4c20fd6d19d38f03: function(arg0, arg1) {
            var v0 = getArrayJsValueFromWasm0(arg0, arg1).slice();
            wasm.__wbindgen_export4(arg0, arg1 * 4, 4);
            console.error(...v0);
        },
        __wbg_error_8d9a8e04cd1d3588: function(arg0) {
            console.error(getObject(arg0));
        },
        __wbg_error_a6fa202b58aa1cd3: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_fetch_5550a88cf343aaa9: function(arg0, arg1) {
            const ret = getObject(arg0).fetch(getObject(arg1));
            return addHeapObject(ret);
        },
        __wbg_fetch_f8a611684c3b5fe5: function(arg0, arg1) {
            const ret = getObject(arg0).fetch(getObject(arg1));
            return addHeapObject(ret);
        },
        __wbg_fillRect_4e5596ca954226e7: function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).fillRect(arg1, arg2, arg3, arg4);
        },
        __wbg_fillText_b1722b6179692b85: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).fillText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_fill_c0bb5e0ec0d7fcf9: function(arg0) {
            getObject(arg0).fill();
        },
        __wbg_from_4bdf88943703fd48: function(arg0) {
            const ret = Array.from(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_fullscreenElement_8068aa5be9c86543: function(arg0) {
            const ret = getObject(arg0).fullscreenElement;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_getBoundingClientRect_b236f2e393fd0e7a: function(arg0) {
            const ret = getObject(arg0).getBoundingClientRect();
            return addHeapObject(ret);
        },
        __wbg_getContext_f04bf8f22dcb2d53: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = getObject(arg0).getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments); },
        __wbg_getElementById_d1f25d287b19a833: function(arg0, arg1, arg2) {
            const ret = getObject(arg0).getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_getItem_a7cc1d4f154f2e6f: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = getObject(arg1).getItem(getStringFromWasm0(arg2, arg3));
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_get_3ef1eba1850ade27: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(getObject(arg0), getObject(arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_get_498b26486bad3f29: function(arg0, arg1) {
            const ret = getObject(arg0)[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_get_a8ee5c45dabc1b3b: function(arg0, arg1) {
            const ret = getObject(arg0)[arg1 >>> 0];
            return addHeapObject(ret);
        },
        __wbg_hash_d4a10bb21e6ca48e: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg1).hash;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_height_6568c4427c3b889d: function(arg0) {
            const ret = getObject(arg0).height;
            return ret;
        },
        __wbg_host_0b682963054cd706: function(arg0) {
            const ret = getObject(arg0).host;
            return addHeapObject(ret);
        },
        __wbg_host_f8ec5a2b33bbf48d: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg1).host;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_innerHeight_ba245c3eff70b2a1: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).innerHeight;
            return addHeapObject(ret);
        }, arguments); },
        __wbg_innerWidth_e6af2d66d3b06991: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).innerWidth;
            return addHeapObject(ret);
        }, arguments); },
        __wbg_insertBefore_64157928ea5f5def: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = getObject(arg0).insertBefore(getObject(arg1), getObject(arg2));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_instanceof_ArrayBuffer_101e2bf31071a9f6: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_CanvasRenderingContext2d_08b9d193c22fa886: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof CanvasRenderingContext2D;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Element_7f177ac0337279af: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof Element;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Error_4691a5b466e32a80: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof Error;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_26125339f936be50: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_KeyboardEvent_b75e17d5aabde4f1: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof KeyboardEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Response_9b4d9fd451e051b1: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof Response;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_ShadowRoot_a69345f84dc5ff1b: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof ShadowRoot;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WheelEvent_7d954cc8de0d5525: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof WheelEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_23e677d2c6843922: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WorkerGlobalScope_de6976d00cb213c6: function(arg0) {
            let result;
            try {
                result = getObject(arg0) instanceof WorkerGlobalScope;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_is_a166b9958c2438ad: function(arg0, arg1) {
            const ret = Object.is(getObject(arg0), getObject(arg1));
            return ret;
        },
        __wbg_key_99eb0f0a1000963d: function(arg0, arg1) {
            const ret = getObject(arg1).key;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_lastChild_c435ca5440ac5b58: function(arg0) {
            const ret = getObject(arg0).lastChild;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_left_0050d4abe2736ee9: function(arg0) {
            const ret = getObject(arg0).left;
            return ret;
        },
        __wbg_length_244965d3c9c88588: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_length_b3416cf66a5452c8: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_length_ea16607d7b61445b: function(arg0) {
            const ret = getObject(arg0).length;
            return ret;
        },
        __wbg_lineTo_8ea7db5b5d763030: function(arg0, arg1, arg2) {
            getObject(arg0).lineTo(arg1, arg2);
        },
        __wbg_listener_id_51707c1ea7d7f75c: function(arg0) {
            const ret = getObject(arg0).__yew_listener_id;
            return isLikeNone(ret) ? 0x100000001 : (ret) >>> 0;
        },
        __wbg_localStorage_51c38b3b222e1ed2: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).localStorage;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        }, arguments); },
        __wbg_location_fc8d47802682dd93: function(arg0) {
            const ret = getObject(arg0).location;
            return addHeapObject(ret);
        },
        __wbg_measureText_a914720e0a913aef: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = getObject(arg0).measureText(getStringFromWasm0(arg1, arg2));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_message_00d63f20c41713dd: function(arg0) {
            const ret = getObject(arg0).message;
            return addHeapObject(ret);
        },
        __wbg_moveTo_6d04ca2f71946754: function(arg0, arg1, arg2) {
            getObject(arg0).moveTo(arg1, arg2);
        },
        __wbg_name_ecf53d5e050a495d: function(arg0) {
            const ret = getObject(arg0).name;
            return addHeapObject(ret);
        },
        __wbg_namespaceURI_0e8590f9609fd689: function(arg0, arg1) {
            const ret = getObject(arg1).namespaceURI;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_naturalWidth_c235f79e9e28f2cc: function(arg0) {
            const ret = getObject(arg0).naturalWidth;
            return ret;
        },
        __wbg_navigator_9cebf56f28aa719b: function(arg0) {
            const ret = getObject(arg0).navigator;
            return addHeapObject(ret);
        },
        __wbg_new_0837727332ac86ba: function() { return handleError(function () {
            const ret = new Headers();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_0b637bad3d58f611: function() { return handleError(function () {
            const ret = new Image();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_227d7c05414eb861: function() {
            const ret = new Error();
            return addHeapObject(ret);
        },
        __wbg_new_5415f704ce1c4eda: function() { return handleError(function () {
            const ret = new URLSearchParams();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_5f486cdf45a04d78: function(arg0) {
            const ret = new Uint8Array(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_new_ab79df5bd7c26067: function() {
            const ret = new Object();
            return addHeapObject(ret);
        },
        __wbg_new_bb1018d527df73cb: function() { return handleError(function (arg0, arg1) {
            const ret = new URL(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_dd50bcc3f60ba434: function() { return handleError(function (arg0, arg1) {
            const ret = new WebSocket(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_ff7542269c97e186: function() { return handleError(function (arg0, arg1) {
            const ret = new MouseEvent(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_with_event_init_dict_fb446c1d36e37046: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = new CloseEvent(getStringFromWasm0(arg0, arg1), getObject(arg2));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_with_str_4c859c3e69e6cb15: function() { return handleError(function (arg0, arg1) {
            const ret = new Request(getStringFromWasm0(arg0, arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_new_with_str_and_init_b4b54d1a819bc724: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = new Request(getStringFromWasm0(arg0, arg1), getObject(arg2));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_nextSibling_afcfdc4e646795ea: function(arg0) {
            const ret = getObject(arg0).nextSibling;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_now_16f0c993d5dd6c27: function() {
            const ret = Date.now();
            return ret;
        },
        __wbg_now_c6d7a7d35f74f6f1: function(arg0) {
            const ret = getObject(arg0).now();
            return ret;
        },
        __wbg_parentElement_6ea1a9ddfe78f22d: function(arg0) {
            const ret = getObject(arg0).parentElement;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_parentNode_f02c28ae3eec09bc: function(arg0) {
            const ret = getObject(arg0).parentNode;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_performance_28be169151161678: function(arg0) {
            const ret = getObject(arg0).performance;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_preventDefault_25a229bfe5c510f8: function(arg0) {
            getObject(arg0).preventDefault();
        },
        __wbg_protocol_2c6bffc6b0fafa07: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg1).protocol;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_prototypesetcall_d62e5099504357e6: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), getObject(arg2));
        },
        __wbg_queueMicrotask_0c399741342fb10f: function(arg0) {
            const ret = getObject(arg0).queueMicrotask;
            return addHeapObject(ret);
        },
        __wbg_queueMicrotask_a082d78ce798393e: function(arg0) {
            queueMicrotask(getObject(arg0));
        },
        __wbg_readyState_1f1e7f1bdf9f4d42: function(arg0) {
            const ret = getObject(arg0).readyState;
            return ret;
        },
        __wbg_reason_cbcb9911796c4714: function(arg0, arg1) {
            const ret = getObject(arg1).reason;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_removeAttribute_c0738b49de4ead0b: function() { return handleError(function (arg0, arg1, arg2) {
            getObject(arg0).removeAttribute(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_removeChild_dfd4207a6ece49c1: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg0).removeChild(getObject(arg1));
            return addHeapObject(ret);
        }, arguments); },
        __wbg_removeEventListener_8ba966e6a7606bce: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).removeEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), arg4 !== 0);
        }, arguments); },
        __wbg_removeEventListener_d27694700fc0df8b: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            getObject(arg0).removeEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3));
        }, arguments); },
        __wbg_removeItem_95c258b9afdd7580: function() { return handleError(function (arg0, arg1, arg2) {
            getObject(arg0).removeItem(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_requestAnimationFrame_206c97f410e7a383: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg0).requestAnimationFrame(getObject(arg1));
            return ret;
        }, arguments); },
        __wbg_requestFullscreen_045a0765d640e198: function() { return handleError(function (arg0) {
            getObject(arg0).requestFullscreen();
        }, arguments); },
        __wbg_resolve_ae8d83246e5bcc12: function(arg0) {
            const ret = Promise.resolve(getObject(arg0));
            return addHeapObject(ret);
        },
        __wbg_search_35617fb7936183df: function(arg0, arg1) {
            const ret = getObject(arg1).search;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_send_4a1dc66e8653e5ed: function() { return handleError(function (arg0, arg1, arg2) {
            getObject(arg0).send(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_send_d31a693c975dea74: function() { return handleError(function (arg0, arg1, arg2) {
            getObject(arg0).send(getArrayU8FromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_setAttribute_f20d3b966749ab64: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setInterval_84b64f01452a246e: function() { return handleError(function (arg0, arg1) {
            const ret = setInterval(getObject(arg0), arg1);
            return addHeapObject(ret);
        }, arguments); },
        __wbg_setItem_5f84aeef0d7f3c17: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).setItem(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setTimeout_ef24d2fc3ad97385: function() { return handleError(function (arg0, arg1) {
            const ret = setTimeout(getObject(arg0), arg1);
            return addHeapObject(ret);
        }, arguments); },
        __wbg_set_7eaa4f96924fd6b3: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
            return ret;
        }, arguments); },
        __wbg_set_binaryType_3dcf8281ec100a8f: function(arg0, arg1) {
            getObject(arg0).binaryType = __wbindgen_enum_BinaryType[arg1];
        },
        __wbg_set_cache_key_274f2145fcb4390c: function(arg0, arg1) {
            getObject(arg0).__yew_subtree_cache_key = arg1 >>> 0;
        },
        __wbg_set_capture_271d6acb719615b3: function(arg0, arg1) {
            getObject(arg0).capture = arg1 !== 0;
        },
        __wbg_set_checked_682ea4d0bea94b97: function(arg0, arg1) {
            getObject(arg0).checked = arg1 !== 0;
        },
        __wbg_set_code_fd32f14824f6885a: function(arg0, arg1) {
            getObject(arg0).code = arg1;
        },
        __wbg_set_fillStyle_1f65027a07e93e62: function(arg0, arg1) {
            getObject(arg0).fillStyle = getObject(arg1);
        },
        __wbg_set_fillStyle_58417b6b548ae475: function(arg0, arg1, arg2) {
            getObject(arg0).fillStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_font_b038797b3573ae5e: function(arg0, arg1, arg2) {
            getObject(arg0).font = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_globalAlpha_d51aa11e10f40cfc: function(arg0, arg1) {
            getObject(arg0).globalAlpha = arg1;
        },
        __wbg_set_hash_de8bb9e7bce63dec: function() { return handleError(function (arg0, arg1, arg2) {
            getObject(arg0).hash = getStringFromWasm0(arg1, arg2);
        }, arguments); },
        __wbg_set_headers_3c8fecc693b75327: function(arg0, arg1) {
            getObject(arg0).headers = getObject(arg1);
        },
        __wbg_set_height_b6548a01bdcb689a: function(arg0, arg1) {
            getObject(arg0).height = arg1 >>> 0;
        },
        __wbg_set_imageSmoothingEnabled_f9f883202f4f3d5e: function(arg0, arg1) {
            getObject(arg0).imageSmoothingEnabled = arg1 !== 0;
        },
        __wbg_set_innerHTML_97039584c4ab4c83: function(arg0, arg1, arg2) {
            getObject(arg0).innerHTML = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_lineCap_1ecf6c7ca9319eb2: function(arg0, arg1, arg2) {
            getObject(arg0).lineCap = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_lineJoin_2c56d0d6bec26d27: function(arg0, arg1, arg2) {
            getObject(arg0).lineJoin = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_lineWidth_e38550ed429ec417: function(arg0, arg1) {
            getObject(arg0).lineWidth = arg1;
        },
        __wbg_set_listener_id_3046a6d6a2394ad9: function(arg0, arg1) {
            getObject(arg0).__yew_listener_id = arg1 >>> 0;
        },
        __wbg_set_method_8c015e8bcafd7be1: function(arg0, arg1, arg2) {
            getObject(arg0).method = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_nodeValue_a32aaa3615b2da17: function(arg0, arg1, arg2) {
            getObject(arg0).nodeValue = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_once_617be4b8bd597c38: function(arg0, arg1) {
            getObject(arg0).once = arg1 !== 0;
        },
        __wbg_set_passive_9e672435b71b9c78: function(arg0, arg1) {
            getObject(arg0).passive = arg1 !== 0;
        },
        __wbg_set_reason_b5edb0791e7766e3: function(arg0, arg1, arg2) {
            getObject(arg0).reason = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_search_bd09fe57b201bac5: function(arg0, arg1, arg2) {
            getObject(arg0).search = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_src_f257a96103ac1ac6: function(arg0, arg1, arg2) {
            getObject(arg0).src = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_strokeStyle_a5baa9565d8b6485: function(arg0, arg1, arg2) {
            getObject(arg0).strokeStyle = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_subtree_id_fc80ace73ff247a7: function(arg0, arg1) {
            getObject(arg0).__yew_subtree_id = arg1 >>> 0;
        },
        __wbg_set_value_0756834ee6cb3709: function(arg0, arg1, arg2) {
            getObject(arg0).value = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_value_d621a056e7640299: function(arg0, arg1, arg2) {
            getObject(arg0).value = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_width_c0fcaa2da53cd540: function(arg0, arg1) {
            getObject(arg0).width = arg1 >>> 0;
        },
        __wbg_slice_6c91edc1cff27a67: function(arg0, arg1) {
            const ret = getObject(arg1).slice();
            const ptr1 = passArrayJsValueToWasm0(ret, wasm.__wbindgen_export);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
            const ret = getObject(arg1).stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_static_accessor_GLOBAL_8adb955bd33fac2f: function() {
            const ret = typeof global === 'undefined' ? null : global;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_static_accessor_GLOBAL_THIS_ad356e0db91c7913: function() {
            const ret = typeof globalThis === 'undefined' ? null : globalThis;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_static_accessor_SELF_f207c857566db248: function() {
            const ret = typeof self === 'undefined' ? null : self;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_static_accessor_WINDOW_bb9f1ba69d61b386: function() {
            const ret = typeof window === 'undefined' ? null : window;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_strokeRect_2e20ce9870736fad: function(arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).strokeRect(arg1, arg2, arg3, arg4);
        },
        __wbg_strokeText_9f0af270c9810166: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            getObject(arg0).strokeText(getStringFromWasm0(arg1, arg2), arg3, arg4);
        }, arguments); },
        __wbg_stroke_affa71c0888c6f31: function(arg0) {
            getObject(arg0).stroke();
        },
        __wbg_subtree_id_6a3200546ad613b1: function(arg0) {
            const ret = getObject(arg0).__yew_subtree_id;
            return isLikeNone(ret) ? 0x100000001 : (ret) >>> 0;
        },
        __wbg_target_7bc90f314634b37b: function(arg0) {
            const ret = getObject(arg0).target;
            return isLikeNone(ret) ? 0 : addHeapObject(ret);
        },
        __wbg_textContent_75b4506705c8c793: function(arg0, arg1) {
            const ret = getObject(arg1).textContent;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_text_372f5b91442c50f9: function() { return handleError(function (arg0) {
            const ret = getObject(arg0).text();
            return addHeapObject(ret);
        }, arguments); },
        __wbg_then_098abe61755d12f6: function(arg0, arg1) {
            const ret = getObject(arg0).then(getObject(arg1));
            return addHeapObject(ret);
        },
        __wbg_then_9e335f6dd892bc11: function(arg0, arg1, arg2) {
            const ret = getObject(arg0).then(getObject(arg1), getObject(arg2));
            return addHeapObject(ret);
        },
        __wbg_toString_3272fa0dfd05dd87: function(arg0) {
            const ret = getObject(arg0).toString();
            return addHeapObject(ret);
        },
        __wbg_toString_fca8b5e46235cfb4: function(arg0) {
            const ret = getObject(arg0).toString();
            return addHeapObject(ret);
        },
        __wbg_top_378559f0b38a1038: function(arg0) {
            const ret = getObject(arg0).top;
            return ret;
        },
        __wbg_touches_bbc155f11e845656: function(arg0) {
            const ret = getObject(arg0).touches;
            return addHeapObject(ret);
        },
        __wbg_url_b6f96880b733816c: function(arg0, arg1) {
            const ret = getObject(arg1).url;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_userAgent_161a5f2d2a8dee61: function() { return handleError(function (arg0, arg1) {
            const ret = getObject(arg1).userAgent;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_value_567d71719bef8eda: function(arg0, arg1) {
            const ret = getObject(arg1).value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_value_8a368d5467debd73: function(arg0, arg1) {
            const ret = getObject(arg1).value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg_wasClean_69f68dc4ed2d2cc7: function(arg0) {
            const ret = getObject(arg0).wasClean;
            return ret;
        },
        __wbg_width_4d6fc7fecd877217: function(arg0) {
            const ret = getObject(arg0).width;
            return ret;
        },
        __wbg_width_eebf2967f114717c: function(arg0) {
            const ret = getObject(arg0).width;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 26, function: Function { arguments: [Externref], shim_idx: 27, ret: Unit, inner_ret: Some(Unit) }, mutable: false }) -> Externref`.
            const ret = makeClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_810, __wasm_bindgen_func_elem_811);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 26, function: Function { arguments: [Ref(NamedExternref("Event"))], shim_idx: 159, ret: Unit, inner_ret: Some(Unit) }, mutable: false }) -> Externref`.
            const ret = makeClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_810, __wasm_bindgen_func_elem_4383);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 33, function: Function { arguments: [Externref], shim_idx: 111, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1895, __wasm_bindgen_func_elem_1949);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 33, function: Function { arguments: [], shim_idx: 61, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1895, __wasm_bindgen_func_elem_1872);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 71, function: Function { arguments: [Ref(NamedExternref("Event"))], shim_idx: 72, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1860, __wasm_bindgen_func_elem_1861);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000006: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 89, function: Function { arguments: [NamedExternref("CloseEvent")], shim_idx: 91, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1866, __wasm_bindgen_func_elem_1867);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000007: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 89, function: Function { arguments: [NamedExternref("Event")], shim_idx: 91, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1866, __wasm_bindgen_func_elem_1867_6);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000008: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 89, function: Function { arguments: [NamedExternref("MessageEvent")], shim_idx: 91, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1866, __wasm_bindgen_func_elem_1867_7);
            return addHeapObject(ret);
        },
        __wbindgen_cast_0000000000000009: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { dtor_idx: 89, function: Function { arguments: [], shim_idx: 61, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm.__wasm_bindgen_func_elem_1866, __wasm_bindgen_func_elem_1872);
            return addHeapObject(ret);
        },
        __wbindgen_cast_000000000000000a: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return addHeapObject(ret);
        },
        __wbindgen_object_clone_ref: function(arg0) {
            const ret = getObject(arg0);
            return addHeapObject(ret);
        },
        __wbindgen_object_drop_ref: function(arg0) {
            takeObject(arg0);
        },
    };
    return {
        __proto__: null,
        "./client_bg.js": import0,
    };
}

function __wasm_bindgen_func_elem_1872(arg0, arg1) {
    wasm.__wasm_bindgen_func_elem_1872(arg0, arg1);
}

function __wasm_bindgen_func_elem_811(arg0, arg1, arg2) {
    wasm.__wasm_bindgen_func_elem_811(arg0, arg1, addHeapObject(arg2));
}

function __wasm_bindgen_func_elem_4383(arg0, arg1, arg2) {
    try {
        wasm.__wasm_bindgen_func_elem_4383(arg0, arg1, addBorrowedObject(arg2));
    } finally {
        heap[stack_pointer++] = undefined;
    }
}

function __wasm_bindgen_func_elem_1861(arg0, arg1, arg2) {
    try {
        wasm.__wasm_bindgen_func_elem_1861(arg0, arg1, addBorrowedObject(arg2));
    } finally {
        heap[stack_pointer++] = undefined;
    }
}

function __wasm_bindgen_func_elem_1867(arg0, arg1, arg2) {
    wasm.__wasm_bindgen_func_elem_1867(arg0, arg1, addHeapObject(arg2));
}

function __wasm_bindgen_func_elem_1867_6(arg0, arg1, arg2) {
    wasm.__wasm_bindgen_func_elem_1867_6(arg0, arg1, addHeapObject(arg2));
}

function __wasm_bindgen_func_elem_1867_7(arg0, arg1, arg2) {
    wasm.__wasm_bindgen_func_elem_1867_7(arg0, arg1, addHeapObject(arg2));
}

function __wasm_bindgen_func_elem_1949(arg0, arg1, arg2) {
    try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.__wasm_bindgen_func_elem_1949(retptr, arg0, arg1, addHeapObject(arg2));
        var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
        var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
        if (r1) {
            throw takeObject(r0);
        }
    } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
    }
}


const __wbindgen_enum_BinaryType = ["blob", "arraybuffer"];

function addHeapObject(obj) {
    if (heap_next === heap.length) heap.push(heap.length + 1);
    const idx = heap_next;
    heap_next = heap[idx];

    heap[idx] = obj;
    return idx;
}

function addBorrowedObject(obj) {
    if (stack_pointer == 1) throw new Error('out of js stack');
    heap[--stack_pointer] = obj;
    return stack_pointer;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => state.dtor(state.a, state.b));

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function dropObject(idx) {
    if (idx < 1028) return;
    heap[idx] = heap_next;
    heap_next = idx;
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(takeObject(mem.getUint32(i, true)));
    }
    return result;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getObject(idx) { return heap[idx]; }

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        wasm.__wbindgen_export3(addHeapObject(e));
    }
}

let heap = new Array(1024).fill(undefined);
heap.push(undefined, null, true, false);

let heap_next = heap.length;

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        try {
            return f(state.a, state.b, ...args);
        } finally {
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function makeMutClosure(arg0, arg1, dtor, f) {
    const state = { a: arg0, b: arg1, cnt: 1, dtor };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            state.dtor(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    const mem = getDataViewMemory0();
    for (let i = 0; i < array.length; i++) {
        mem.setUint32(ptr + 4 * i, addHeapObject(array[i]), true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

let stack_pointer = 1024;

function takeObject(idx) {
    const ret = getObject(idx);
    dropObject(idx);
    return ret;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('client_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
