FROM ubuntu:20.04

# updates, and fetch dependency code
RUN apt update && apt-get install -y software-properties-common wget
RUN wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add -
RUN apt-get update
RUN apt-add-repository "deb http://apt.llvm.org/bionic/ llvm-toolchain-bionic-6.0 main"
RUN apt-get install -y clang-6.0 lld-6.0
RUN apt-get install -y automake autoconf libtool cmake git libatomic-ops-dev
RUN cd opt/ && git clone -b vosk --single-branch https://github.com/alphacep/kaldi && cd kaldi/tools/ && \
    git clone -b v0.3.13 --single-branch https://github.com/xianyi/OpenBLAS && \
    git clone -b v3.2.1 --single-branch https://github.com/alphacep/clapack

# building openblas and clapack
RUN cd /opt/kaldi/tools && \
    CXX=clang++-6.0 CC=clang-6.0 make -C OpenBLAS ONLY_CBLAS=1 DYNAMIC_ARCH=1 TARGET=NEHALEM USE_LOCKING=1 USE_THREAD=0 all && \
    CXX=clang++-6.0 CC=clang-6.0 make -C OpenBLAS PREFIX=$(pwd)/OpenBLAS/install install && \
    mkdir -p clapack/BUILD && \
    cd clapack/BUILD/ && \
    CXX=clang++-6.0 CC=clang-6.0 cmake ..
RUN cd /opt/kaldi/tools/clapack/BUILD && \
    CXX=clang++-6.0 CC=clang-6.0 make -j 10 && \
    find . -name "*.a" | xargs cp -t ../../OpenBLAS/install/lib

# building openfst
RUN cd /opt/kaldi/tools/ && \
    git clone --single-branch https://github.com/alphacep/openfst openfst && \
    cd openfst/ && \
    autoreconf -i && \
    CXX=clang++-6.0 CC=clang-6.0 CFLAGS="-g -O3" ./configure --prefix=/opt/kaldi/tools/openfst \
    --enable-static --enable-shared --enable-far --enable-ngram-fsts --enable-lookahead-fsts --with-pic --disable-bin && \
    CXX=clang++-6.0 CC=clang-6.0 make -j 10 && \
    CXX=clang++-6.0 CC=clang-6.0 make install

# building kaldi
RUN cd /opt/kaldi/src/ && \
    CXX=clang++-6.0 CC=clang-6.0 ./configure --mathlib=OPENBLAS_CLAPACK --shared --use-cuda=no && \
    sed -i 's:-msse -msse2:-msse -msse2:g' kaldi.mk && \
    sed -i 's: -O1 : -O3 :g' kaldi.mk && \
    CXX=clang++-6.0 CC=clang-6.0 make -j $(nproc) online2 lm rnnlm

    
# building vosk api and creating libvosk.so
RUN cd /opt && \
    git clone https://github.com/alphacep/vosk-api && \
    cd /opt/vosk-api/src/ && \
    clang++-6.0 -g -O3 -std=c++17 -Wno-deprecated-declarations -fPIC -DFST_NO_DYNAMIC_LINKING -I. -I/opt/kaldi/src -I/opt/kaldi/tools/openfst/include -I/opt/kaldi/tools/OpenBLAS/install/include \
    -c -o recognizer.o recognizer.cc && \
    clang++-6.0 -g -O3 -std=c++17 -Wno-deprecated-declarations -fPIC -DFST_NO_DYNAMIC_LINKING -I. -I/opt/kaldi/src -I/opt/kaldi/tools/openfst/include -I/opt/kaldi/tools/OpenBLAS/install/include \
    -c -o language_model.o language_model.cc && \
    clang++-6.0 -g -O3 -std=c++17 -Wno-deprecated-declarations -fPIC -DFST_NO_DYNAMIC_LINKING -I. -I/opt/kaldi/src -I/opt/kaldi/tools/openfst/include -I/opt/kaldi/tools/OpenBLAS/install/include \
    -c -o model.o model.cc && \
    clang++-6.0 -g -O3 -std=c++17 -Wno-deprecated-declarations -fPIC -DFST_NO_DYNAMIC_LINKING -I. -I/opt/kaldi/src -I/opt/kaldi/tools/openfst/include -I/opt/kaldi/tools/OpenBLAS/install/include \
    -c -o spk_model.o spk_model.cc && \
    clang++-6.0 -g -O3 -std=c++17 -Wno-deprecated-declarations -fPIC -DFST_NO_DYNAMIC_LINKING -I. -I/opt/kaldi/src -I/opt/kaldi/tools/openfst/include -I/opt/kaldi/tools/OpenBLAS/install/include \
    -c -o vosk_api.o vosk_api.cc && \
    clang++-6.0 -shared -o libvosk.so recognizer.o language_model.o model.o spk_model.o vosk_api.o 


# Copy libvosk.so to a target directory
RUN mkdir -p /opt/vosk-api/src/copydir && \
    cp /opt/vosk-api/src/libvosk.so /opt/vosk-api/src/copydir && \
    cp /opt/vosk-api/src/vosk_api.h /opt/vosk-api/src/copydir && \
    ls /opt/vosk-api/src/copydir



