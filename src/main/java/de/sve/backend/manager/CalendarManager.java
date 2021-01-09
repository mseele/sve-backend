package de.sve.backend.manager;

import java.io.IOException;
import java.security.GeneralSecurityException;
import java.time.LocalDateTime;
import java.util.List;

import com.google.api.services.calendar.model.Channel;

import de.sve.backend.calendar.CalendarService;
import de.sve.backend.model.calendar.Appointment;

public class CalendarManager {

	private static final String GENERAL_ID = "info@sv-eutingen.de"; //$NON-NLS-1$

	private static final String WATCH_ID = "01234567-89ab-cdef-0123456789ab"; //$NON-NLS-1$

	private static final String WATCH_RESOURCE_ID = "9-xc9GFSc2LvPpsJiw8HveIDA3c"; //$NON-NLS-1$


	public static List<Appointment> appointments() throws GeneralSecurityException, IOException {
		return CalendarService.appointments(GENERAL_ID, 100);
	}

	public static Channel renewWatch() throws IOException, GeneralSecurityException {
		CalendarService.stop(WATCH_ID, WATCH_RESOURCE_ID);
		return CalendarService.watch(GENERAL_ID, WATCH_ID, LocalDateTime.now().plusYears(1));
	}

}
