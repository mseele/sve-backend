package de.sve.backend.manager;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.util.List;

import de.sve.backend.calendar.CalendarService;
import de.sve.backend.model.calendar.Appointment;

public class CalendarManager {

	private static final String GENERAL_ID = "info@sv-eutingen.de"; //$NON-NLS-1$

//	CalendarService.watch(GENERAL_ID, "01234567-89ab-cdef-0123456789ab", LocalDateTime.of(2030, 1, 1, 0, 0));
//	{
//	   "expiration":"1599763064000",
//	   "id":"01234567-89ab-cdef-0123456789ab",
//	   "kind":"api#channel",
//	   "resourceId":"9-xc9GFSc2LvPpsJiw8HveIDA3c",
//	   "resourceUri":"https://www.googleapis.com/calendar/v3/calendars/info@sv-eutingen.de/events?alt=json"
//	}

	public static List<Appointment> appointments() throws GeneralSecurityException, IOException {
		return CalendarService.appointments(GENERAL_ID, 100);
	}

}
