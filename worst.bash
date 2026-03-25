bin_path="./target/worst/debug"
result=$("$bin_path" $*)
if [ -n "$result" ] && [ -z "${result##EXEC::*}" ]; then
   eval "${result#EXEC::}"
else
   echo "$result"
fi
